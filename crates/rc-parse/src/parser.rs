//! Moteur de parsing : pile de scopes et parsing incrémental par ligne.
//!
//! L'algorithme suit TextMate (et vscode-textmate) :
//!
//! * l'état entre deux lignes est une **pile de nœuds** ([`Node`]) — chaque
//!   nœud est une règle begin/end entrée (commentaire, chaîne …), portant
//!   le scope de son contenu ;
//! * une ligne est parcourue de gauche à droite. À chaque position on tente
//!   d'abord les règles `end` **de la pile, de l'intérieur vers
//!   l'extérieur** : toutes celles dont le `end` matche à cette position
//!   ferment ensemble (un `*/` referme les commentaires imbriqués), puis
//!   les règles du nœud courant (ou racine), dans l'ordre du fichier — la
//!   première qui matche à la position gagne ;
//! * les règles `match`/`begin` sont compilées **ancrées par `\G`** : elles
//!   ne peuvent matcher qu'à la position courante (sémantique TextMate) ;
//! * les plages émises sont en **caractères** (les positions onig sont en
//!   octets — conversion à l'émission) ;
//! * aucune règle ne matche → on avance d'un caractère UTF-8 ; le texte
//!   « muet » s'accumule et est émis au prochain événement sous le scope
//!   courant, ainsi qu'en fin de ligne.
//!
//! **Incremental** : [`Parser::parse_from`] reparse depuis la première ligne
//! modifiée et s'arrête dès que la pile produite pour une ligne égale celle
//! déjà enregistrée. L'invariant de TextMate (même pile d'entrée ⇒ mêmes
//! scopes de sortie) garantit que tout ce qui suit est inchangé.

use std::ops::Range;
use std::sync::Arc;

use onig::{Region, SearchOptions};
use rc_buffer::Buffer;
use rc_scope::Scope;

use crate::grammar::{Captures, Grammar, PatternItem, RuleIndex, RuleKind, MAX_INCLUDE_DEPTH};

/// Un nœud de la pile : une règle begin/end entrée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// Index de la règle dans l'arène de la grammaire.
    pub rule: RuleIndex,
    /// Scope du contenu (`contentName`, sinon `name`), déjà résolu.
    content_scope: Option<String>,
}

/// Une plage scopée : `range` en **caractères** de la ligne courante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedRange {
    pub scope: Scope,
    pub range: Range<usize>,
}

/// Le parseur incrémental d'un document pour une grammaire donnée.
#[derive(Debug)]
pub struct Parser {
    grammar: Arc<Grammar>,
    /// `stack_after[i]` = pile **après** la ligne i.
    stack_after: Vec<Vec<Node>>,
    /// `ranges[i]` = paires (scope, plage) de la ligne i.
    ranges: Vec<Vec<ScopedRange>>,
    /// Dernière ligne réellement parsée (`None` : rien encore).
    parsed_through: Option<usize>,
}

/// Borne le nombre d'itérations par ligne (garde anti-boucles pathologiques).
const GUARD_MAX: u32 = 20_000;

impl Parser {
    /// Un parseur vide pour `grammar` (rien de parsé).
    pub fn new(grammar: Arc<Grammar>) -> Self {
        Self {
            grammar,
            stack_after: Vec::new(),
            ranges: Vec::new(),
            parsed_through: None,
        }
    }

    /// Consomme une grammaire et retourne un parseur vide.
    pub fn from_grammar(g: Grammar) -> Self {
        Self::new(Arc::new(g))
    }

    // =====================
    // = Parsing incrémental =
    // =====================

    /// Reparse le buffer depuis la première ligne modifiée (`first_changed`)
    /// et retourne le nombre de lignes réellement retravaillées.
    ///
    /// Règles :
    /// * reparse depuis `min(first_changed, première ligne non encore
    ///   parsée)` — ce qui n'a jamais été parsé est repris quel que soit
    ///   l'argument (buffer agrandi en fin de document) ;
    /// * s'arrête dès qu'une ligne produit exactement la même pile que celle
    ///   déjà enregistrée : tout ce qui suit est inchangé ;
    /// * les tableaux internes sont tronqués si le buffer a rétréci.
    pub fn parse_from(&mut self, buffer: &Buffer, first_changed: usize) -> usize {
        let line_count = buffer.line_count();
        if self.stack_after.len() > line_count {
            self.stack_after.truncate(line_count);
            self.ranges.truncate(line_count);
        }
        if let Some(p) = self.parsed_through {
            if p >= line_count {
                self.parsed_through = Some(line_count.saturating_sub(1));
            }
        }

        let start_raw = first_changed.min(line_count.saturating_sub(1));
        let start = match self.parsed_through {
            None => start_raw,
            Some(p) => start_raw.min(p.saturating_add(1)),
        };

        // Pile d'entrée : celle qui suit la ligne précédente (déjà parsée).
        let mut stack: Vec<Node> = if start == 0 {
            Vec::new()
        } else {
            self.stack_after[start - 1].clone()
        };

        let mut parsed = 0;
        let mut i = start;
        while i < line_count {
            let line = buffer.line(i);
            let (next, rngs) = self.parse_line(stack, &line);
            // Point d'arrêt incremental : la pile produite égale celle déjà
            // enregistrée ⇒ tout ce qui suit est inchangé. La ligne courante
            // est néanmoins ENREGISTRÉE : elle peut avoir changé de contenu
            // tout en produisant la même pile (texte "muet" remplacé par
            // d'autre texte muet) — ses plages viennent d'être recalculées.
            let stable = self.stack_after.get(i) == Some(&next);
            if self.stack_after.len() <= i {
                self.stack_after.push(next.clone());
                self.ranges.push(Vec::new());
            }
            self.stack_after[i] = next.clone();
            self.ranges[i] = rngs;
            stack = next;
            parsed += 1;
            i += 1;
            if stable {
                break;
            }
        }

        if parsed > 0 {
            self.parsed_through = Some(i - 1);
        }
        parsed
    }

    /// Parse tout le buffer (équivalent `parse_from(buffer, 0)`).
    pub fn parse_all(&mut self, buffer: &Buffer) -> usize {
        self.parse_from(buffer, 0)
    }

    // =====================
    // = Accès aux résultats =
    // =====================

    /// Les paires (scope, plage) de la ligne `i` (vide si non parsée).
    pub fn line_ranges(&self, i: usize) -> &[ScopedRange] {
        self.ranges.get(i).map(Vec::as_slice).unwrap_or(&[])
    }

    /// La pile de scopes **après** la ligne `i`.
    pub fn stack_after(&self, i: usize) -> &[Node] {
        self.stack_after.get(i).map(Vec::as_slice).unwrap_or(&[])
    }

    /// La grammaire utilisée.
    pub fn grammar(&self) -> &Grammar {
        &self.grammar
    }

    // =====================
    // = Parse d'une ligne =
    // =====================

    /// Parse `line` depuis la pile `stack` (résultat de la ligne précédente).
    /// Retourne (nouvelle pile, plages scopées en caractères).
    fn parse_line(&self, mut stack: Vec<Node>, line: &str) -> (Vec<Node>, Vec<ScopedRange>) {
        let mut ranges: Vec<ScopedRange> = Vec::new();
        let bytes_len = line.len();
        let mut pos: usize = 0; // position courante, en octets
        let mut emitted: usize = 0; // fin du dernier range émis, en caractères
        let mut guard: u32 = 0;

        while pos < bytes_len && guard < GUARD_MAX {
            guard += 1;

            // 1) Fermeture : toutes les règles dont l'`end` matche à cette
            //    position, de l'intérieur vers l'extérieur.
            if !stack.is_empty() {
                if let Some(new_pos) = self.try_close(&mut stack, line, pos, &mut ranges, &mut emitted) {
                    pos = new_pos;
                    continue;
                }
            }

            // 2) Règles du nœud courant (ou racine si pile vide).
            let pattern_source = stack
                .last()
                .map(|n| n.rule)
                .unwrap_or_else(|| self.grammar.root());
            let mut candidates: Vec<RuleIndex> = Vec::new();
            let rule = self.grammar.rule(pattern_source);
            self.collect_patterns(&rule.patterns, 0, &mut candidates);

            let pos_before = pos;
            for &cand in &candidates {
                let cand_rule = self.grammar.rule(cand);
                match &cand_rule.kind {
                    RuleKind::Container => unreachable!("conteneurs expansés à la collecte"),
                    RuleKind::Match { re, captures } => {
                        if let Some((s, e, region)) = self.search_caps(*re, line, pos) {
                            debug_assert_eq!(s, pos, "règle match non ancrée");
                            if s == e {
                                continue; // match vide : règle suivante
                            }
                            let base = self.scope_path(&stack);
                            self.push_region(
                                &mut ranges, &mut emitted,
                                &base, cand_rule.name.as_deref(),
                                line, s, e, captures, &region,
                            );
                            pos = e;
                            break;
                        }
                    }
                    RuleKind::BeginEnd { begin, begin_captures, .. } => {
                        if let Some((s, e, region)) = self.search_caps(*begin, line, pos) {
                            debug_assert_eq!(s, pos, "règle begin non ancrée");
                            if s == e {
                                // begin vide : on entre quand même, mais on
                                // avance d'un caractère (garde anti-boucle).
                                stack.push(Node {
                                    rule: cand,
                                    content_scope: cand_rule.content_scope.clone(),
                                });
                                self.advance_char(line, &mut pos);
                                break;
                            }
                            let base = self.scope_path(&stack);
                            let extra = begin_captures
                                .scope_for(0)
                                .or(cand_rule.name.as_deref());
                            self.push_region(
                                &mut ranges, &mut emitted,
                                &base, extra,
                                line, s, e, begin_captures, &region,
                            );
                            stack.push(Node {
                                rule: cand,
                                content_scope: cand_rule.content_scope.clone(),
                            });
                            pos = e;
                            break;
                        }
                    }
                }
            }

            if pos == pos_before {
                // Rien ne matche : on avance d'un caractère UTF-8.
                self.advance_char(line, &mut pos);
            }
        }

        // Fermeture possible à la toute dernière position de la ligne :
        // les fins `end` zero-width (`$`, lookahead de fin de ligne) ne
        // peuvent matcher qu'ici, là où la boucle ci-dessus s'arrête.
        if !stack.is_empty() {
            self.try_close(&mut stack, line, pos, &mut ranges, &mut emitted);
        }

        // Reliquat de ligne sous le scope courant.
        let base = self.scope_path(&stack);
        self.push_gap(&mut ranges, &mut emitted, &base, line, line.len());
        (stack, ranges)
    }

    /// Ferme de l'intérieur vers l'extérieur tous les nœuds dont l'`end`
    /// matche à la position `pos` (même début de match que le plus interne).
    /// Retourne la nouvelle position, ou `None` si rien ne ferme.
    fn try_close(
        &self,
        stack: &mut Vec<Node>,
        line: &str,
        pos: usize,
        ranges: &mut Vec<ScopedRange>,
        emitted: &mut usize,
    ) -> Option<usize> {
        let mut first: Option<(usize, usize)> = None; // (s, e) du plus interne
        let mut closing: Vec<(usize, Region)> = Vec::new(); // (position de pile, région), interne→externe

        for idx in (0..stack.len()).rev() {
            let rule = self.grammar.rule(stack[idx].rule);
            if let RuleKind::BeginEnd { end: Some(end_re), .. } = &rule.kind {
                if let Some((s, e, region)) = self.search_caps(*end_re, line, pos) {
                    match first {
                        None => {
                            first = Some((s, e));
                            closing.push((idx, region));
                        }
                        Some((first_s, _)) if s == first_s => closing.push((idx, region)),
                        Some(_) => break, // les plus externes ferment plus loin
                    }
                }
            }
        }

        let (s, e) = first?;

        // 1) Le contenu jusqu'au début de la fin (gap) est scopé avec la
        //    pile COMPLÈTE (les nœuds sont encore ouverts sur ce texte).
        let base_full = self.scope_path(stack);
        self.push_gap(ranges, emitted, &base_full, line, s);

        // 2) Pour chaque nœud fermé, du plus interne au plus externe : pop
        //    puis portion `end` scopée avec la pile DONT le nœud vient
        //    d'être retiré (comme vscode-textmate) — sinon `name` y
        //    figurerait deux fois (à la fois nœud et extra).
        for (idx, region) in closing {
            let rule = self.grammar.rule(stack[idx].rule);
            let end_caps = match &rule.kind {
                RuleKind::BeginEnd { end_captures, .. } => end_captures.clone(),
                _ => Captures::default(),
            };
            stack.truncate(idx); // ferme ce nœud et tous ceux au-dessus
            let base = self.scope_path(stack);
            let extra = end_caps.scope_for(0).or(rule.name.as_deref());
            self.push_region(ranges, emitted, &base, extra, line, s, e, &end_caps, &region);
        }
        Some(e)
    }

    /// Émet : (1) le gap [emitted, s) sous `base`, (2) la région [s, e) sous
    /// `base` + `extra` (+ sous-captures), en caractères.
    #[allow(clippy::too_many_arguments)]
    fn push_region(
        &self,
        ranges: &mut Vec<ScopedRange>,
        emitted: &mut usize,
        base: &str,
        extra: Option<&str>,
        line: &str,
        s_byte: usize,
        e_byte: usize,
        caps: &Captures,
        region: &Region,
    ) {
        let s = byte_to_char(line, s_byte);
        let e = byte_to_char(line, e_byte);
        self.push_gap(ranges, emitted, base, line, s_byte);
        if e > s {
            let scope = Scope::parse(&Self::join(base, extra));
            ranges.push(ScopedRange { scope, range: s..e });
            *emitted = e;
        }
        // Sous-captures (groupes 1+) : scopes supplémentaires sur leurs plages.
        let base_extra = Self::join(base, extra);
        for (group, cap_scope) in caps.entries() {
            if let Some((gs_b, ge_b)) = region.pos(*group) {
                let gs = byte_to_char(line, gs_b);
                let ge = byte_to_char(line, ge_b);
                let sub = format!("{base_extra} {cap_scope}");
                ranges.push(ScopedRange { scope: Scope::parse(&sub), range: gs..ge });
            }
        }
    }

    /// Émet le gap `[emitted, until_byte)` sous `base`, en caractères.
    fn push_gap(
        &self,
        ranges: &mut Vec<ScopedRange>,
        emitted: &mut usize,
        base: &str,
        line: &str,
        until_byte: usize,
    ) {
        let until = byte_to_char(line, until_byte);
        if until > *emitted {
            let scope = Scope::parse(base);
            ranges.push(ScopedRange { scope, range: *emitted..until });
            *emitted = until;
        }
    }

    /// Le scope courant (grammaire + pile) sous forme de chaîne.
    fn scope_path(&self, stack: &[Node]) -> String {
        let mut s = String::new();
        s.push_str(&self.grammar.scope_name);
        for n in stack {
            if let Some(cs) = &n.content_scope {
                s.push(' ');
                s.push_str(cs);
            }
        }
        s
    }

    fn join(base: &str, extra: Option<&str>) -> String {
        match extra {
            None => base.to_string(),
            Some(x) if base.is_empty() => x.to_string(),
            Some(x) => format!("{base} {x}"),
        }
    }

    /// Expansion des `patterns` d'une règle (résolution inline des
    /// conteneurs et des `include`), bornée en profondeur.
    ///
    /// Sémantique TextMate : `include: "#cle"` insère la règle du
    /// repository **comme motif à part entière** (son `match`/`begin`
    /// s'applique) ; un conteneur (règle sans motif propre) est étalé
    /// inline ; `include: "$self"` ré-insère les patterns racine.
    fn collect_patterns(&self, items: &[PatternItem], depth: u32, out: &mut Vec<RuleIndex>) {
        if depth > MAX_INCLUDE_DEPTH {
            return;
        }
        for it in items {
            let idx = match it {
                PatternItem::Rule(r) | PatternItem::Include(r) => *r,
                PatternItem::Root => {
                    let root = self.grammar.root();
                    let patterns = &self.grammar.rule(root).patterns;
                    self.collect_patterns(patterns, depth + 1, out);
                    continue;
                }
            };
            let rule = self.grammar.rule(idx);
            if matches!(rule.kind, RuleKind::Container) {
                self.collect_patterns(&rule.patterns, depth + 1, out);
            } else {
                out.push(idx);
            }
        }
    }

    /// Recherche (+ captures en positions d'octets) : ancrée si la regex
    /// contient `\G` (compilée ainsi pour `match`/`begin`), libre sinon.
    fn search_caps(&self, re_idx: usize, line: &str, at: usize) -> Option<(usize, usize, Region)> {
        let re = self.grammar.regex(re_idx);
        let mut region = Region::new();
        let start = re.search_with_options(line, at, line.len(), SearchOptions::SEARCH_OPTION_NONE, Some(&mut region))?;
        let end = region
            .pos(0)
            .map(|(_, e)| e)
            .unwrap_or(start);
        Some((start, end, region))
    }

    fn advance_char(&self, line: &str, pos: &mut usize) {
        if *pos < line.len() {
            match line[*pos..].chars().next() {
                Some(c) => *pos += c.len_utf8(),
                None => *pos = line.len(),
            }
        } else {
            *pos = line.len();
        }
    }
}

fn byte_to_char(line: &str, byte: usize) -> usize {
    line[..byte].chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(json: &str) -> Grammar {
        Grammar::from_json_str(json).expect("grammaire JSON")
    }

    const SIMPLE: &str = r#"{
      "scopeName": "source.demo",
      "patterns": [
        { "match": "\\b(foo|bar)\\b", "name": "keyword.control",
          "captures": { "1": { "name": "entity.name" } } }
      ]
    }"#;

    fn scopes(ranges: &[ScopedRange]) -> Vec<String> {
        ranges.iter().map(|r| r.scope.to_string()).collect()
    }

    #[test]
    fn simple_match_ranges() {
        let mut p = Parser::from_grammar(g(SIMPLE));
        let b = Buffer::from_str("foo bar baz");
        p.parse_all(&b);
        let r = p.line_ranges(0);
        // foo → région keyword + sous-capture, bar idem, gap avant bar, baz nu
        let got: Vec<(Range<usize>, String)> =
            r.iter().map(|x| (x.range.clone(), x.scope.to_string())).collect();
        assert_eq!(
            got,
            vec![
                (0..3, "source.demo keyword.control".into()),
                (0..3, "source.demo keyword.control entity.name".into()),
                (3..4, "source.demo".into()),
                (4..7, "source.demo keyword.control".into()),
                (4..7, "source.demo keyword.control entity.name".into()),
                (7..11, "source.demo".into()),
            ]
        );
    }

    #[test]
    fn capture_sub_range_scope() {
        let mut p = Parser::from_grammar(g(SIMPLE));
        let b = Buffer::from_str("foo");
        p.parse_all(&b);
        let r = p.line_ranges(0);
        assert_eq!(
            scopes(r),
            vec![
                "source.demo keyword.control",
                "source.demo keyword.control entity.name"
            ]
        );
    }

    #[test]
    fn begin_end_single_line() {
        let mut p = Parser::from_grammar(g(
            r#"{
              "scopeName": "source.demo",
              "patterns": [
                { "begin": "/\\*", "end": "\\*/", "name": "comment.block" }
              ]
            }"#,
        ));
        let b = Buffer::from_str("a /* x */ b");
        p.parse_all(&b);
        let r = p.line_ranges(0);
        let got: Vec<(Range<usize>, String)> =
            r.iter().map(|x| (x.range.clone(), x.scope.to_string())).collect();
        assert_eq!(
            got,
            vec![
                (0..2, "source.demo".into()),
                (2..4, "source.demo comment.block".into()),
                (4..7, "source.demo comment.block".into()),
                (7..9, "source.demo comment.block".into()),
                (9..11, "source.demo".into()),
            ]
        );
    }

    #[test]
    fn begin_end_multiline_carries_stack() {
        let mut p = Parser::from_grammar(g(
            r#"{
              "scopeName": "source.demo",
              "patterns": [
                { "begin": "/\\*", "end": "\\*/", "name": "comment.block" }
              ]
            }"#,
        ));
        let b = Buffer::from_str("/* open\nstill comment\nend */");
        p.parse_all(&b);
        assert_eq!(p.stack_after(0).len(), 1);
        assert_eq!(p.stack_after(0)[0].content_scope.as_deref(), Some("comment.block"));
        let r1 = p.line_ranges(1);
        assert_eq!(r1.len(), 1);
        assert_eq!(r1[0].scope.to_string(), "source.demo comment.block");
        assert_eq!(r1[0].range, 0..13); // "still comment" = 13 caractères
        assert_eq!(p.stack_after(2).len(), 0);
    }

    #[test]
    fn nested_comments_close_together() {
        let mut p = Parser::from_grammar(g(
            r#"{
              "scopeName": "source.demo",
              "patterns": [
                { "begin": "/\\*", "end": "\\*/", "name": "comment.block",
                  "patterns": [ { "include": "$self" } ] }
              ]
            }"#,
        ));
        let b = Buffer::from_str("/* a /* b */");
        p.parse_all(&b);
        assert_eq!(p.stack_after(0).len(), 0, "le */ referme les deux niveaux");
        assert!(scopes(p.line_ranges(0))
            .iter()
            .any(|s| s == "source.demo comment.block comment.block"));
    }

    #[test]
    fn string_with_escape_patterns() {
        let mut p = Parser::from_grammar(g(
            r#"{
              "scopeName": "source.demo",
              "patterns": [
                { "begin": "\"", "end": "\"", "name": "string.quoted.double",
                  "patterns": [ { "match": "\\\\[nt]", "name": "constant.character.escape" } ] }
              ]
            }"#,
        ));
        let b = Buffer::from_str("\"a\\tb\"");
        p.parse_all(&b);
        assert!(scopes(p.line_ranges(0)).iter().any(
            |s| s == "source.demo string.quoted.double constant.character.escape"
        ));
    }

    #[test]
    fn include_repository() {
        let mut p = Parser::from_grammar(g(
            r##"{
              "scopeName": "source.demo",
              "repository": {
                "str": { "begin": "\"", "end": "\"", "name": "string.quoted.double" }
              },
              "patterns": [ { "include": "#str" } ]
            }"##,
        ));
        let b = Buffer::from_str("\"hi\" there");
        p.parse_all(&b);
        assert!(scopes(p.line_ranges(0)).iter().any(|s| s == "source.demo string.quoted.double"));
    }

    #[test]
    fn incremental_stops_early() {
        let mut p = Parser::from_grammar(g(SIMPLE));
        let mut lines = String::new();
        for i in 0..100 {
            lines.push_str(if i % 2 == 0 { "foo\n" } else { "bar\n" });
        }
        let mut b = Buffer::from_str(&lines);
        // 100 lignes de contenu + la ligne vide finale (newline terminal)
        assert_eq!(p.parse_all(&b), 101);

        // Edit de la ligne 40 : seule elle est re-parsée (la pile de la 41
        // redevient identique → arrêt).
        let s = b.line_to_char(40);
        let e = b.line_to_char(41);
        b.replace(s..e, "baz");
        assert_eq!(p.parse_from(&b, 40), 1);
        assert_eq!(p.line_ranges(40)[0].scope.to_string(), "source.demo");
        // Les lignes suivantes sont intactes.
        assert_eq!(p.line_ranges(41)[0].scope.to_string(), "source.demo keyword.control");
    }

    #[test]
    fn incremental_respects_unparsed_tail() {
        let mut p = Parser::from_grammar(g(SIMPLE));
        let mut b = Buffer::from_str("foo\n");
        p.parse_all(&b);
        b.insert(b.len_chars(), "bar\n");
        // La nouvelle ligne n'a jamais été parsée : reprise automatique.
        assert_eq!(p.parse_from(&b, 1), 1);
        assert_eq!(p.line_ranges(1)[0].scope.to_string(), "source.demo keyword.control");
    }

    #[test]
    fn unicode_positions_are_chars() {
        let mut p = Parser::from_grammar(g(
            r#"{
              "scopeName": "source.demo",
              "patterns": [ { "match": "héllo", "name": "keyword" } ]
            }"#,
        ));
        let b = Buffer::from_str("héllo 🎉");
        p.parse_all(&b);
        let r = p.line_ranges(0);
        assert_eq!(r[0].range, 0..5);
        assert_eq!(r[0].scope.to_string(), "source.demo keyword");
        assert_eq!(r[1].range, 5..7);
        assert_eq!(r[1].scope.to_string(), "source.demo");
    }

    #[test]
    fn shrink_truncates_stacks() {
        let mut p = Parser::from_grammar(g(SIMPLE));
        let mut b = Buffer::from_str("foo\nfoo\nfoo\n");
        p.parse_all(&b);
        assert!(p.stack_after(2).len() <= 1);
        // Deux lignes supprimées.
        b.remove(b.line_to_char(1)..b.len_chars());
        assert_eq!(p.parse_from(&b, 0), 1);
        assert!(p.line_ranges(0).len() >= 1);
    }

    #[test]
    fn match_not_at_position_is_ignored_and_advance_happens() {
        let mut p = Parser::from_grammar(g(
            r#"{
              "scopeName": "source.demo",
              "patterns": [ { "match": "AB", "name": "keyword" } ]
            }"#,
        ));
        let b = Buffer::from_str("xAB");
        p.parse_all(&b);
        let r = p.line_ranges(0);
        assert_eq!(r[0].range, 0..1);
        assert_eq!(r[0].scope.to_string(), "source.demo");
        assert_eq!(r[1].scope.to_string(), "source.demo keyword");
        assert_eq!(r[1].range, 1..3);
    }

    #[test]
    fn line_comment_ends_at_eol() {
        // `end: "$"` : la fin de commentaire de ligne est en fin de ligne
        // (zero-width) — doit fermer après la boucle, pas persister.
        let mut p = Parser::from_grammar(g(
            r#"{
              "scopeName": "source.demo",
              "patterns": [
                { "begin": "//", "end": "$", "name": "comment.line" },
                { "match": "\\bfoo\\b", "name": "keyword" }
              ]
            }"#,
        ));
        let b = Buffer::from_str("foo // note
foo");
        p.parse_all(&b);
        let scopes0 = scopes(p.line_ranges(0));
        let scopes1 = scopes(p.line_ranges(1));
        // ligne 0 : foo nu + com de « // » au EOL ; ligne 1 : foo seul, MUET
        assert!(scopes0.iter().any(|s| s == "source.demo"));
        assert!(scopes0.iter().any(|s| s == "source.demo comment.line"));
        assert_eq!(p.stack_after(0).len(), 0, "commentaire de ligne fermé");
        assert!(scopes1.iter().any(|s| s == "source.demo keyword"));
    }

    #[test]
    fn xml_plist_grammar_end_to_end() {
        // Grammaire C-like en plist XML : entraîne le chemin complet
        // (plist → arène → parsing) avec begin/end et règles match.
        let xml = r##"<?xml version="1.0"?>
<plist version="1.0">
<dict>
  <key>scopeName</key><string>source.clike</string>
  <key>patterns</key>
  <array>
    <dict>
      <key>begin</key><string>/\*</string>
      <key>end</key><string>\*/</string>
      <key>name</key><string>comment.block</string>
    </dict>
    <dict>
      <key>match</key><string>\b(int|void|return)\b</string>
      <key>name</key><string>keyword.control</string>
    </dict>
  </array>
</dict>
</plist>"##;
        let g = Grammar::from_plist_str(xml).unwrap();
        let mut p = Parser::from_grammar(g);
        let b = Buffer::from_str("int main /* x */ { return 0; }");
        p.parse_all(&b);
        let scopes = scopes(p.line_ranges(0));
        assert!(scopes.iter().any(|s| s == "source.clike keyword.control"),
            "int/return → keyword : {scopes:?}");
        assert!(scopes.iter().any(|s| s == "source.clike comment.block"),
            "commentaire de bloc : {scopes:?}");
        assert_eq!(p.stack_after(0).len(), 0, "tout refermé");
    }

    #[test]
    fn escaped_quote_does_not_close_string() {
        // Un guillemet échappé ne referme pas la chaîne : l'échappement
        // est consommé par le motif avant que le `end` ne soit testé.
        let mut p = Parser::from_grammar(g(
            r#"{
              "scopeName": "source.demo",
              "patterns": [
                { "begin": "\"", "end": "\"", "name": "string.quoted.double",
                  "patterns": [ { "match": "\\\\.", "name": "constant.character.escape" } ] }
              ]
            }"#,
        ));
        let b = Buffer::from_str("\"a\\\"b\"");
        p.parse_all(&b);
        let scopes = scopes(p.line_ranges(0));
        // le \" échappé est un escape ; la chaîne se ferme au 2e guillemet
        assert!(scopes.iter().any(|s| s.contains("constant.character.escape")));
        assert!(scopes.iter().any(|s| s == "source.demo string.quoted.double"));
        assert_eq!(p.stack_after(0).len(), 0);
    }

    #[test]
    fn empty_line_keeps_stack() {
        let mut p = Parser::from_grammar(g(
            r#"{
              "scopeName": "source.demo",
              "patterns": [
                { "begin": "/\\*", "end": "\\*/", "name": "comment.block" }
              ]
            }"#,
        ));
        let b = Buffer::from_str("/*\n\n*/");
        p.parse_all(&b);
        // la ligne vide reste dans le commentaire
        assert_eq!(p.stack_after(0).len(), 1);
        assert_eq!(p.stack_after(1).len(), 1);
        assert_eq!(p.stack_after(2).len(), 0);
    }
}
