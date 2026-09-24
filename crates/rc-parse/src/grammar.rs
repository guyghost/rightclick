//! Modèle de grammaire : chargement `.tmLanguage` + arène de règles.
//!
//! Le format TextMate est chargé tel quel :
//!
//! ```xml
//! <dict>
//!   <key>scopeName</key>  <string>source.demo</string>
//!   <key>patterns</key>   <array>…</array>
//!   <key>repository</key> <dict>…</dict>
//! </dict>
//! ```
//!
//! Les motifs reposent sur Oniguruma (crate `onig`) : les règles
//! `match`/`begin` sont compilées **ancrées** par `\G` (elles doivent
//! matcher exactement à la position courante), les règles `end` à
//! l'inverse ne le sont pas (elles se cherchent plus loin dans la ligne).
//!
//! Les `include` (`"#cle"`, `"$self"`, `"$base"`) sont résolus en
//! [`PatternItem`] à la construction — l'expansion récursive a lieu à la
//! parse, bornée en profondeur pour garantir la terminaison sur les
//! grammaires qui s'incluent elles-mêmes (listes Markdown, etc.).

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use onig::Regex;
use serde::Deserialize;

/// Erreur de chargement ou de construction d'une grammaire.
#[derive(Debug)]
pub enum GrammarError {
    Io(std::io::Error),
    Plist(String),
    Json(String),
    Regex(String),
}

impl fmt::Display for GrammarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GrammarError::Io(e) => write!(f, "lecture impossible : {e}"),
            GrammarError::Plist(e) => write!(f, "plist invalide : {e}"),
            GrammarError::Json(e) => write!(f, "JSON invalide : {e}"),
            GrammarError::Regex(e) => write!(f, "regex invalide : {e}"),
        }
    }
}

impl From<std::io::Error> for GrammarError {
    fn from(e: std::io::Error) -> Self {
        GrammarError::Io(e)
    }
}

/// Index d'une règle dans l'arène [`Grammar::rules`].
pub type RuleIndex = usize;

/// Une grammaire compilée, prête à parser.
#[derive(Debug)]
pub struct Grammar {
    /// Nom lisible (clé `name`, optionnelle).
    pub name: String,
    /// Scope racine (`scopeName`), ex. `"source.rust"`.
    pub scope_name: String,
    /// Arène de règles ; l'index [`Grammar::root`] est la règle racine.
    rules: Vec<Rule>,
    /// Règle racine (conteneur synthétique des `patterns` de haut niveau).
    root: RuleIndex,
    /// Résolution `repository` : clé → index de règle.
    repo: HashMap<String, RuleIndex>,
    /// Moteurs regex compilés, référencés par index.
    regexes: Vec<Regex>,
}

/// Une règle (élément de `patterns` ou entrée de `repository`).
#[derive(Debug)]
pub struct Rule {
    /// Scope `name` : s'applique à tout le match (ou begin/end).
    pub name: Option<String>,
    /// Scope du contenu (`contentName`, sinon `name`) — utilisé comme
    /// scope de la progression entre begin et end.
    pub content_scope: Option<String>,
    /// Le comportement de la règle.
    pub kind: RuleKind,
    /// Règles imbriquées (contenu de begin/end, ou règles d'un
    /// conteneur). Une règle `include` est déjà résolue ici.
    pub patterns: Vec<PatternItem>,
    /// Index de la règle d'origine dans l'arène (utile au débogage).
    pub index: RuleIndex,
}

/// Le comportement d'une règle.
#[derive(Debug)]
pub enum RuleKind {
    /// `match` : un motif, des captures. Ne pousse jamais de pile.
    Match { re: usize, captures: Captures },
    /// `begin`/`end` : un état de pile ; `end == None` signifie « ne se
    /// referme jamais » (cas des règles `match`+`patterns`, qui entrent
    /// un état persistant).
    BeginEnd {
        begin: usize,
        end: Option<usize>,
        begin_captures: Captures,
        end_captures: Captures,
    },
    /// Conteneur : pas de motif propre, uniquement des `patterns`
    /// (expansion inline à la position courante).
    Container,
}

/// Un élément de la liste de motifs d'une règle.
#[derive(Debug, Clone)]
pub enum PatternItem {
    /// Une sous-règle locale (imbriquée dans `patterns`).
    Rule(RuleIndex),
    /// Les `patterns` d'une règle du repository (`include: "#cle"`).
    Include(RuleIndex),
    /// Les `patterns` racine (`include: "$self"` / `"$base"`).
    Root,
}

/// Les captures d'une règle : numéro de groupe → scope.
///
/// La capture 0 (le match entier) n'est pas stockée ici — elle est portée
/// par `name` / `content_name`.
#[derive(Debug, Clone, Default)]
pub struct Captures {
    entries: Vec<(usize, String)>,
}

impl Captures {
    /// Le scope affecté au groupe `group`, s'il existe.
    pub fn scope_for(&self, group: usize) -> Option<&str> {
        self.entries
            .iter()
            .find(|(g, _)| *g == group)
            .map(|(_, s)| s.as_str())
    }

    /// Les paires (groupe, scope), triées par groupe.
    pub fn entries(&self) -> &[(usize, String)] {
        &self.entries
    }
}

/// DTO de désérialisation — format TextMate brut.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGrammar {
    name: Option<String>,
    scope_name: Option<String>,
    #[serde(default)]
    patterns: Vec<RawRule>,
    #[serde(default)]
    repository: Option<HashMap<String, RawRule>>,
}

/// DTO d'une règle brute.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRule {
    name: Option<String>,
    content_name: Option<String>,
    #[serde(rename = "match")]
    match_: Option<String>,
    begin: Option<String>,
    end: Option<String>,
    #[serde(rename = "while")]
    while_: Option<String>,
    #[serde(default)]
    captures: Option<HashMap<String, RawCapture>>,
    #[serde(default)]
    begin_captures: Option<HashMap<String, RawCapture>>,
    #[serde(default)]
    end_captures: Option<HashMap<String, RawCapture>>,
    #[serde(default)]
    patterns: Vec<RawRule>,
    include: Option<String>,
}

/// DTO d'une entrée de captures.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCapture {
    name: Option<String>,
}

impl Grammar {
    /// Charge une grammaire depuis un fichier `.tmLanguage`.
    ///
    /// Le format est détecté : JSON (premier caractère `{`) sinon plist
    /// (XML ou binaire).
    pub fn load(path: impl AsRef<Path>) -> Result<Self, GrammarError> {
        let bytes = std::fs::read(path)?;
        if bytes.iter().copied().find(|b| !b.is_ascii_whitespace()) == Some(b'{') {
            let s = std::str::from_utf8(&bytes).map_err(|e| GrammarError::Json(e.to_string()))?;
            Self::from_json_str(s)
        } else {
            Self::from_plist_bytes(&bytes)
        }
    }

    /// Construit une grammaire depuis des octets plist (XML ou binaire).
    pub fn from_plist_bytes(bytes: &[u8]) -> Result<Self, GrammarError> {
        let raw: RawGrammar = plist::from_bytes(bytes).map_err(|e| GrammarError::Plist(e.to_string()))?;
        Self::from_raw(raw)
    }

    /// Construit une grammaire depuis une chaîne plist XML.
    pub fn from_plist_str(s: &str) -> Result<Self, GrammarError> {
        let raw: RawGrammar = plist::from_bytes(s.as_bytes())
            .map_err(|e| GrammarError::Plist(e.to_string()))?;
        Self::from_raw(raw)
    }

    /// Construit une grammaire depuis une chaîne JSON (tmLanguage récents).
    pub fn from_json_str(s: &str) -> Result<Self, GrammarError> {
        let raw: RawGrammar = serde_json::from_str(s).map_err(|e| GrammarError::Json(e.to_string()))?;
        Self::from_raw(raw)
    }

    fn from_raw(raw: RawGrammar) -> Result<Self, GrammarError> {
        let mut g = Self {
            name: raw.name.clone().unwrap_or_else(|| raw.scope_name.clone().unwrap_or_default()),
            scope_name: raw.scope_name.clone().unwrap_or_default(),
            rules: Vec::new(),
            root: 0,
            repo: HashMap::new(),
            regexes: Vec::new(),
        };

        // Règle racine synthétique (conteneur) puis règles de haut niveau.
        g.root = g.push_container();
        let mut pending_includes: Vec<(RuleIndex, String)> = Vec::new();
        for r in &raw.patterns {
            let idx = g.add_rule(r, &mut pending_includes)?;
            g.rules[g.root].patterns.push(PatternItem::Rule(idx));
        }

        // Repository : clé → règle (ordre trié pour la déterminisme).
        let mut keys: Vec<&String> = raw
            .repository
            .as_ref()
            .map(|r| r.keys().collect())
            .unwrap_or_default();
        keys.sort();
        for key in &keys {
            let r = &raw.repository.as_ref().unwrap()[*key];
            let idx = g.add_rule(r, &mut pending_includes)?;
            g.repo.insert((*key).clone(), idx);
        }

        // Résolution des include (étape 2 : le repo est complet).
        for (rule_idx, inc) in &pending_includes {
            let item = match inc.as_str() {
                "#" => {
                    // `"#"` = les patterns racine (équivalent $self).
                    PatternItem::Root
                }
                _ if inc.starts_with('#') => {
                    let key = &inc[1..];
                    match g.repo.get(key) {
                        Some(&idx) => PatternItem::Include(idx),
                        None => continue, // repository introuvable : ignoré
                    }
                }
                "$self" | "$base" => PatternItem::Root,
                _ => continue, // autre syntaxe (cross-grammaire ?) : ignoré
            };
            g.rules[*rule_idx].patterns.push(item);
        }

        if g.scope_name.is_empty() {
            return Err(GrammarError::Plist(
                "clé `scopeName` absente : ce n'est pas une grammaire TextMate".into(),
            ));
        }
        Ok(g)
    }

    /// Construit la règle (arène) et l'ajoute à l'arène.
    fn add_rule(&mut self, raw: &RawRule, pending: &mut Vec<(RuleIndex, String)>) -> Result<RuleIndex, GrammarError> {
        // Règles imbriquées (patterns directs), construites d'abord.
        let mut patterns = Vec::new();
        for r in &raw.patterns {
            let child = self.add_rule(r, pending)?;
            patterns.push(PatternItem::Rule(child));
        }
        // Index calculé APRÈS les enfants : c'est maintenant que la place
        // de la règle courante se crée dans l'arène.
        let idx = self.rules.len();
        // Include éventuel (résolu à l'étape 2, quand le repo est complet).
        if let Some(inc) = &raw.include {
            pending.push((idx, inc.clone()));
        }

        // Le scope du contenu : `contentName` prime, sinon `name`.
        let content_scope = raw.content_name.clone().or_else(|| raw.name.clone());

        // Les règles `while` ne sont pas (encore) prises en charge : on ne
        // les entre jamais, on ne garde que leurs patterns (documenté).
        let kind = if raw.while_.is_some() {
            RuleKind::Container
        } else if let Some(m) = &raw.match_ {
            // Règle `match`. Avec `patterns` = état persistant : converti en
            // begin/end sans `end` (son contenu garde la scope au-delà du match).
            if raw.patterns.is_empty() {
                RuleKind::Match {
                    re: self.compile_anchored(m)?,
                    captures: Self::parse_captures(raw.captures.as_ref()),
                }
            } else {
                RuleKind::BeginEnd {
                    begin: self.compile_anchored(m)?,
                    end: None,
                    begin_captures: Self::parse_captures(raw.captures.as_ref()),
                    end_captures: Captures::default(),
                }
            }
        } else if let Some(b) = &raw.begin {
            let end_re = raw.end.as_ref().map(|e| self.compile_anchored(e)).transpose()?;
            RuleKind::BeginEnd {
                begin: self.compile_anchored(b)?,
                end: end_re,
                begin_captures: Self::parse_captures(raw.begin_captures.as_ref()),
                end_captures: Self::parse_captures(raw.end_captures.as_ref()),
            }
        } else {
            RuleKind::Container
        };

        self.rules.push(Rule {
            name: raw.name.clone(),
            content_scope,
            kind,
            patterns,
            index: idx,
        });
        Ok(idx)
    }

    /// Compile une regex **ancrée** (`match`/`begin`) : préfixe `\G`.
    ///
    /// `\G` ancre la recherche à la position de départ passée à
    /// `onig::Regex::search` — la règle ne peut matcher qu'à la position
    /// courante du parseur, exactement la sémantique TextMate.
    fn compile_anchored(&mut self, pattern: &str) -> Result<usize, GrammarError> {
        let re = Regex::new(&format!(r"\G(?:{pattern})"))
            .map_err(|e| GrammarError::Regex(format!("{pattern:?} : {e}")))?;
        let idx = self.regexes.len();
        self.regexes.push(re);
        Ok(idx)
    }

    /// Compile une regex **libre** (`end`) : cherchée plus loin dans la
    /// ligne.
    /// Compile une regex **libre**. Non utilisée pour l'instant (toutes les
    /// règles, `end` compris, sont ancrées) ; conservée pour une future
    /// recherche anticipée des fins de blocs très longs.
    #[allow(dead_code)]
    fn compile_search(&mut self, pattern: &str) -> Result<usize, GrammarError> {
        let re = Regex::new(pattern).map_err(|e| GrammarError::Regex(format!("{pattern:?} : {e}")))?;
        let idx = self.regexes.len();
        self.regexes.push(re);
        Ok(idx)
    }

    /// Parse le dictionnaire de captures `{"1": {name…}}`.
    /// Le groupe 0 (match entier) est conservé : il désigne le scope du
    /// begin/end entier (surcharge `name`/`contentName` côté TextMate).
    fn parse_captures(map: Option<&HashMap<String, RawCapture>>) -> Captures {
        let mut entries: Vec<(usize, String)> = Vec::new();
        if let Some(map) = map {
            for (key, cap) in map {
                if let (Ok(group), Some(name)) = (key.parse::<usize>(), &cap.name) {
                    entries.push((group, name.clone()));
                }
            }
        }
        entries.sort_by_key(|(g, _)| *g);
        Captures { entries }
    }

    fn push_container(&mut self) -> RuleIndex {
        let idx = self.rules.len();
        self.rules.push(Rule {
            name: None,
            content_scope: None,
            kind: RuleKind::Container,
            patterns: Vec::new(),
            index: idx,
        });
        idx
    }

    // =====================
    // = Accès internes (parser) =
    // =====================

    /// La règle d'index `idx`.
    pub(crate) fn rule(&self, idx: RuleIndex) -> &Rule {
        &self.rules[idx]
    }

    /// La règle racine.
    pub(crate) fn root(&self) -> RuleIndex {
        self.root
    }

    /// Le moteur regex d'index `idx`.
    pub(crate) fn regex(&self, idx: usize) -> &Regex {
        &self.regexes[idx]
    }

    /// Le scope racine sous forme de chaîne (pour tests).
    pub fn root_scope(&self) -> &str {
        &self.scope_name
    }
}

/// Profondeur maximale d'expansion des `include` / conteneurs.
///
/// Borne la récursion des grammaires qui s'incluent elles-mêmes
/// (ex. Markdown : le motif de liste inclut `$self`). Au-delà, l'inclusion
/// est ignorée au lieu de boucler.
pub const MAX_INCLUDE_DEPTH: u32 = 32;

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>name</key><string>Demo</string>
  <key>scopeName</key><string>source.demo</string>
  <key>patterns</key>
  <array>
    <dict>
      <key>match</key><string>\b(foo|bar)\b</string>
      <key>name</key><string>keyword.control</string>
      <key>captures</key>
      <dict>
        <key>1</key><dict><key>name</key><string>entity.name</string></dict>
      </dict>
    </dict>
    <dict>
      <key>begin</key><string>/\*</string>
      <key>end</key><string>\*/</string>
      <key>name</key><string>comment.block</string>
    </dict>
    <dict>
      <key>begin</key><string>"</string>
      <key>end</key><string>"</string>
      <key>beginCaptures</key>
      <dict><key>0</key><dict><key>name</key><string>punctuation.definition.string.begin</string></dict></dict>
      <key>endCaptures</key>
      <dict><key>0</key><dict><key>name</key><string>punctuation.definition.string.end</string></dict></dict>
      <key>name</key><string>string.quoted.double</string>
      <key>patterns</key>
      <array>
        <dict>
          <key>match</key><string>\\[nt]</string>
          <key>name</key><string>constant.character.escape</string>
        </dict>
      </array>
    </dict>
  </array>
</dict>
</plist>"#;

    #[test]
    fn loads_xml_plist() {
        let g = Grammar::from_plist_str(XML).unwrap();
        assert_eq!(g.name, "Demo");
        assert_eq!(g.scope_name, "source.demo");
        // racine + 3 règles de haut niveau + la règle imbriquée (escape)
        assert_eq!(g.rules.len(), 5);
        let root = g.rule(g.root());
        assert_eq!(root.patterns.len(), 3);
    }

    #[test]
    fn loads_json_variant() {
        let json = r#"{ "scopeName": "source.j", "patterns": [ { "match": "x", "name": "keyword" } ] }"#;
        let g = Grammar::from_json_str(json).unwrap();
        assert_eq!(g.scope_name, "source.j");
        assert_eq!(g.rule(g.root()).patterns.len(), 1);
    }

    #[test]
    fn error_when_no_scope_name() {
        let g = Grammar::from_json_str(r#"{ "patterns": [] }"#);
        assert!(g.is_err());
    }

    #[test]
    fn repo_include_resolution() {
        let json = r##"{
          "scopeName": "source.r",
          "repository": { "kw": { "match": "\\bTODO\\b", "name": "comment.todo" } },
          "patterns": [ { "include": "#kw" } ]
        }"##;
        let g = Grammar::from_json_str(json).unwrap();
        let root = g.rule(g.root());
        // La règle `include: "#kw"` devient un CONTENEUR (pas de motif
        // propre) ; sa résolution d'include pointe vers la règle du repo.
        assert_eq!(root.patterns.len(), 1);
        let container = match &root.patterns[0] {
            PatternItem::Rule(idx) => g.rule(*idx),
            other => panic!("attendait Rule, trouvé {other:?}"),
        };
        assert!(matches!(container.kind, RuleKind::Container));
        match &container.patterns[0] {
            PatternItem::Include(idx) => {
                let r = g.rule(*idx);
                assert_eq!(r.name.as_deref(), Some("comment.todo"));
                assert!(matches!(r.kind, RuleKind::Match { .. }));
            }
            other => panic!("attendait Include, trouvé {other:?}"),
        }
    }

    #[test]
    fn match_with_patterns_becomes_persistent_state() {
        let json = r#"{
          "scopeName": "source.p",
          "patterns": [ {
            "match": "///",
            "name": "comment.line",
            "patterns": [ { "match": "\\bTODO\\b", "name": "comment.todo" } ]
          } ]
        }"#;
        let g = Grammar::from_json_str(json).unwrap();
        let root = g.rule(g.root());
        let idx = match &root.patterns[0] {
            PatternItem::Rule(idx) => *idx,
            other => panic!("{other:?}"),
        };
        let r = g.rule(idx);
        match &r.kind {
            RuleKind::BeginEnd { end, .. } => assert!(end.is_none(), "match+patterns → pas de end"),
            other => panic!("attendait BeginEnd, trouvé {other:?}"),
        }
    }

    #[test]
    fn self_include_does_not_loop_at_build() {
        let json = r##"{
          "scopeName": "source.s",
          "patterns": [ { "include": "$self" }, { "match": "a", "name": "keyword" } ]
        }"##;
        // Construit sans boucler (la récursion est bornée au moment du parse)
        let g = Grammar::from_json_str(json).unwrap();
        // include $self résolu en Root + la règle match
        assert_eq!(g.rule(g.root()).patterns.len(), 2);
    }
}
