//! Le modèle document : buffer + caret + undo/redo + propreté.
//!
//! L'historique est **basé sur des records** comme chez TextMate : chaque
//! édition produit un [`Record`] décrivant la transformation appliquée
//! (`range` remplacé par `inserted`, avec `removed` conservé pour l'inverse).
//! Les records consécutifs compatibles sont groupés en une [`Step`] :
//!
//! * frappe continue → un seul groupe (Ctrl+Z annule toute la phrase) ;
//! * Retour arrière / Suppr maintenus → un seul groupe ;
//! * déplacement du caret, annulation, refaire, ouverture → rupture.
//!
//! Comme dans TextMate, un groupe est borné ([`DEFAULT_GROUP_CAP`]) pour que
//! l'annulation d'une frappe ininterrompue reste instantanée.

use std::cell::Cell;
use std::ops::Range;

use rc_buffer::Buffer;

use crate::encoding::Encoding;
use crate::format::FileFormat;

/// Nombre maximal de records par groupe d'undo (garde-fou anti-paluche).
pub const DEFAULT_GROUP_CAP: usize = 512;

/// Un document : buffer + caret + historique d'édition.
#[derive(Debug)]
pub struct Document {
    buffer: Buffer,
    caret: usize,
    format: FileFormat,
    undo_stack: Vec<Step>,
    redo_stack: Vec<Step>,
    clean_hash: Option<u64>,
    /// Cache (révision observée, hash observé) pour éviter de re-hasher
    /// tout le document à chaque appel [`Document::is_clean`] (la barre
    /// d'état interroge la propreté sans emprunt mutable).
    clean_cache: Cell<Option<(u64, u64)>>,
    /// Faux après un mouvement de caret / une annulation / un chargement :
    /// le prochain record crée un nouveau groupe.
    merge_enabled: bool,
    group_cap: usize,
}

/// Un groupe d'édition annulable d'un geste (1+ records, même geste).
#[derive(Debug)]
pub struct Step {
    records: Vec<Record>,
    caret_before: usize,
    caret_after: usize,
}

/// Une transformation élémentaire du buffer.
#[derive(Debug)]
struct Record {
    /// Plage remplacée, en coordonnées du buffer **avant** application.
    range: Range<usize>,
    /// Texte présent avant (la plage), conservé pour l'inverse.
    removed: String,
    /// Texte inséré à la place.
    inserted: String,
}

impl Document {
    /// Un document vide (encodage UTF-8 par défaut).
    pub fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            caret: 0,
            format: FileFormat::for_text(""),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clean_hash: None,
            clean_cache: Cell::new(None),
            merge_enabled: false,
            group_cap: DEFAULT_GROUP_CAP,
        }
    }

    /// Un document depuis du texte (encodage UTF-8).
    pub fn from_text(text: &str) -> Self {
        let mut doc = Self::new();
        doc.buffer = Buffer::from_str(text);
        doc.format = FileFormat::for_text(text);
        doc
    }

    /// Un document depuis des octets : l'encodage est détecté (BOM +
    /// heuristiques, voir [`encoding::detect`]) puis le contenu décodé.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let (text, format) = FileFormat::decode(bytes);
        let mut doc = Self::from_text(&text);
        doc.format = format;
        doc
    }

    // =====================
    // = Lecture ===========
    // =====================

    /// Le contenu complet sous forme de `String`.
    pub fn text(&self) -> String {
        self.buffer.text()
    }

    /// Le nombre de caractères du buffer.
    pub fn len_chars(&self) -> usize {
        self.buffer.len_chars()
    }

    /// Le nombre de lignes (un buffer vide compte 1 ligne, comme TextMate).
    pub fn line_count(&self) -> usize {
        self.buffer.line_count()
    }

    /// La ligne `line_idx` (0-based), sans sa fin de ligne.
    pub fn line(&self, line_idx: usize) -> String {
        self.buffer.line(line_idx)
    }

    /// La position actuelle du caret (en caractères).
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// L'encodage détecté / utilisé pour la prochaine sauvegarde.
    pub fn encoding(&self) -> Encoding {
        self.format.encoding()
    }

    /// Encodage et style de fin de ligne conservés pour ce document.
    pub fn format(&self) -> FileFormat {
        self.format
    }

    /// Le numéro de révision du buffer (voir [`Buffer::revision`]).
    pub fn revision(&self) -> u64 {
        self.buffer.revision()
    }

    // =====================
    // = Édition ===========
    // =====================

    /// Déplace le caret (borné au contenu). Rompt le groupe en cours :
    /// taper juste après un déplacement commence une nouvelle Step.
    pub fn set_caret(&mut self, pos: usize) {
        self.caret = pos.min(self.buffer.len_chars());
        self.merge_enabled = false;
    }

    /// Insère `text` au caret puis avance le caret.
    pub fn insert(&mut self, text: &str) {
        let caret_before = self.caret;
        let pos = self.caret;
        self.buffer.insert(pos, text);
        self.caret = pos + text.chars().count();
        let record =
            Record { range: pos..pos, removed: String::new(), inserted: text.to_string() };
        self.push_record(record, caret_before);
    }

    /// Supprime le caractère avant le caret (`Retour arrière`).
    /// Renvoie `false` si le caret était au début du buffer.
    pub fn delete_backward(&mut self) -> bool {
        if self.caret == 0 {
            return false;
        }
        let caret_before = self.caret;
        let range = (self.caret - 1)..self.caret;
        let removed = self.buffer.slice(range.clone());
        self.buffer.remove(range.clone());
        self.caret = range.start;
        self.push_record(Record { range, removed, inserted: String::new() }, caret_before);
        true
    }

    /// Supprime le caractère après le caret (`Suppr`).
    /// Renvoie `false` si le caret était en fin de buffer.
    pub fn delete_forward(&mut self) -> bool {
        let len = self.buffer.len_chars();
        if self.caret >= len {
            return false;
        }
        let caret_before = self.caret;
        let range = self.caret..(self.caret + 1);
        let removed = self.buffer.slice(range.clone());
        self.buffer.remove(range.clone());
        self.push_record(Record { range, removed, inserted: String::new() }, caret_before);
        true
    }

    /// Remplace la plage `range` par `text` (sélection écrasée) et place le
    /// caret à la fin du texte inséré.
    pub fn replace_range(&mut self, range: Range<usize>, text: &str) {
        let start = range.start.min(self.buffer.len_chars());
        let end = range.end.min(self.buffer.len_chars());
        let range = start..end;
        let caret_before = start;
        let removed = self.buffer.slice(range.clone());
        self.buffer.replace(range.clone(), text);
        self.caret = start + text.chars().count();
        let record = Record { range, removed, inserted: text.to_string() };
        self.push_record(record, caret_before);
    }

    // =====================
    // = Undo / Redo =======
    // =====================

    /// Annule le dernier groupe. Restaure le caret au début du groupe.
    pub fn undo(&mut self) {
        let Some(step) = self.undo_stack.pop() else { return };
        self.apply_inverse(&step);
        self.caret = step.caret_before;
        self.redo_stack.push(step);
        self.merge_enabled = false;
    }

    /// Refait le dernier groupe annulé. Restaure le caret à sa fin.
    pub fn redo(&mut self) {
        let Some(step) = self.redo_stack.pop() else { return };
        self.apply(&step);
        self.caret = step.caret_after;
        self.undo_stack.push(step);
        self.merge_enabled = false;
    }

    /// Y a-t-il un groupe à annuler ?
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Y a-t-il un groupe à refaire ?
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    // =====================
    // = Propreté ==========
    // =====================

    /// Marque l'état courant comme enregistré (point de propreté).
    pub fn mark_saved(&mut self) {
        let h = fnv1a(self.buffer.text().as_bytes());
        self.clean_hash = Some(h);
        self.clean_cache.set(Some((self.buffer.revision(), h)));
    }

    /// Le document est-il identique à son dernier point d'enregistrement ?
    ///
    /// La comparaison porte sur le **contenu** (hash FNV-1a), pas sur la
    /// révision : l'annulation passe par des mutations du buffer dont la
    /// révision diffère de l'original, alors que le texte, lui, redevient
    /// identique. Un Ctrl+Z qui ramène exactement au contenu enregistré
    /// rend donc le document propre, sans resauvegarder.
    pub fn is_clean(&self) -> bool {
        let Some(saved) = self.clean_hash else {
            return false; // aucun point d'enregistrement (document neuf)
        };
        let hash = match self.clean_cache.get() {
            Some((rev, h)) if rev == self.buffer.revision() => h,
            _ => {
                let h = fnv1a(self.buffer.text().as_bytes());
                self.clean_cache.set(Some((self.buffer.revision(), h)));
                h
            }
        };
        saved == hash
    }

    // =====================
    // = Sauvegarde ========
    // =====================

    /// Le contenu ré-encodé dans l'encodage du document (BOM compris),
    /// prêt à être écrit sur disque par la couche I/O.
    pub fn save_bytes(&self) -> Vec<u8> {
        self.format.encode_text(&self.buffer.text())
    }

    // =====================
    // = Internes ==========
    // =====================

    fn push_record(&mut self, record: Record, caret_before: usize) {
        let merge = self.merge_enabled
            && self.undo_stack.last().is_some_and(|top| {
                top.records.len() < self.group_cap
                    && should_merge(&top.records[top.records.len() - 1], &record)
            });
        if merge {
            // Coalescence dans le groupe en cours (frappe / suppression continue)
            let top = self.undo_stack.last_mut().expect("vérifié ci-dessus");
            top.records.push(record);
            top.caret_after = self.caret;
        } else {
            // Nouveau groupe : invalide la pile redo (la chronologie diverge)
            self.redo_stack.clear();
            self.undo_stack.push(Step {
                records: vec![record],
                caret_before,
                caret_after: self.caret,
            });
        }
        self.merge_enabled = true;
    }

    /// Applique la transformation d'une Step dans le sens normal (redo).
    ///
    /// Chaque record est **rejoué tel quel** (`replace(range, inserted)`) :
    /// sa plage est exprimée dans l'état d'avant le record, et le redo
    /// repart exactement de cet état (après l'undo de la Step, ou avant
    /// son application initiale).
    fn apply(&mut self, step: &Step) {
        for record in &step.records {
            self.buffer.replace(record.range.clone(), &record.inserted);
        }
    }

    /// Applique l'inverse d'une Step (undo), dans l'ordre inverse : après
    /// l'inverse du record k, le buffer est exactement l'état d'avant le
    /// record k, où la plage de k-1 est de nouveau valide.
    fn apply_inverse(&mut self, step: &Step) {
        for record in step.records.iter().rev() {
            let start = record.range.start;
            let end = start + record.inserted.chars().count();
            self.buffer.replace(start..end, &record.removed);
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

/// Deux records consécutifs forment-ils le même geste d'édition ?
fn should_merge(prev: &Record, cur: &Record) -> bool {
    // 1. Frappe continue : deux insertions pures sur la même ligne, à la suite.
    if prev.removed.is_empty()
        && cur.removed.is_empty()
        && !prev.inserted.is_empty()
        && !cur.inserted.is_empty()
    {
        let same_line = !prev.inserted.contains('\n') && !cur.inserted.contains('\n');
        // Le texte de prev occupe `range.start..range.start+len` : cur doit
        // reprendre exactement à la suite (sur des ranges vides,
        // `range.end == range.start`, d'où le calcul explicite).
        let prev_end = prev.range.start + prev.inserted.chars().count();
        return same_line && prev_end == cur.range.start;
    }
    // 2. Suppression continue (Retour arrière ou Suppr maintenus).
    if !prev.removed.is_empty()
        && !cur.removed.is_empty()
        && prev.inserted.is_empty()
        && cur.inserted.is_empty()
    {        let prev_len = prev.removed.chars().count();
        // Retour arrière : cur retire les caractères juste avant la plage de prev.
        let backward =
            cur.range.end == prev.range.start && cur.range.start + prev_len == prev.range.start;
        // Suppr : cur retire les caractères juste après la plage de prev.
        let forward = cur.range.start == prev.range.start;
        return backward || forward;
    }
    false
}

/// Hash FNV-1a 64 bits sur les octets (suffisant pour comparer des
/// contenus de documents ; collision négligeable pour cet usage).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding;

    fn doc(s: &str) -> Document {
        Document::from_text(s)
    }

    #[test]
    fn save_bytes_normalizes_mixed_newlines_and_keeps_the_loaded_encoding() {
        let original = encoding::encode("a\r\nb\n", Encoding::Utf8Bom);
        let document = Document::from_bytes(&original);

        assert_eq!(document.format().newline().as_str(), "\r\n");
        assert_eq!(
            document.save_bytes(),
            encoding::encode("a\r\nb\r\n", Encoding::Utf8Bom)
        );
    }

    #[test]
    fn from_text_uses_lf_for_newlines_inserted_into_a_single_line() {
        let mut document = Document::from_text("single line");
        document.set_caret(document.len_chars());
        document.insert("\nsecond line");

        assert_eq!(document.save_bytes(), b"single line\nsecond line");
    }

    #[test]
    fn save_bytes_keeps_utf16_and_windows_1252_encodings_while_normalizing() {
        for encoding in [Encoding::Utf16Le, Encoding::Utf16Be, Encoding::Windows1252] {
            let original = encoding::encode("café\r\nb\n", encoding);
            let document = Document::from_bytes(&original);
            assert_eq!(
                document.save_bytes(),
                encoding::encode("café\r\nb\r\n", encoding)
            );
        }
    }

    #[test]
    fn typing_coalesces_into_one_group() {
        let mut d = doc("");
        for c in ["a", "b", "c", " ", "d"] {
            d.insert(c);
        }
        assert_eq!(d.text(), "abc d");
        assert!(d.can_undo());
        // toute la frappe = UN groupe : un seul Ctrl+Z
        d.undo();
        assert_eq!(d.text(), "");
        assert!(!d.can_undo());
        d.redo();
        assert_eq!(d.text(), "abc d");
        assert!(!d.can_redo());
    }

    #[test]
    fn caret_move_breaks_group() {
        let mut d = doc("");
        d.insert("a");
        d.set_caret(0);
        d.insert("b");
        // deux groupes distincts : annuler remonte d'un geste à la fois
        assert_eq!(d.text(), "ba");
        d.undo();
        assert_eq!(d.text(), "a");
        d.undo();
        assert_eq!(d.text(), "");
    }

    #[test]
    fn backspace_coalesces_after_typing_group() {
        let mut d = doc("");
        for c in ["h", "e", "l", "l", "o"] {
            d.insert(c);
        }
        // typage = groupe 1 ; 2 retours arrière = groupe 2 (coalescés)
        assert!(d.delete_backward());
        assert!(d.delete_backward());
        assert_eq!(d.text(), "hel");
        d.undo();
        assert_eq!(d.text(), "hello"); // défait les 2 backspaces ensemble
        d.undo();
        assert_eq!(d.text(), ""); // défait la frappe
        assert!(!d.can_undo());
    }

    #[test]
    fn forward_delete_coalesces() {
        let mut d = doc("abcde");
        d.set_caret(0);
        assert!(d.delete_forward());
        assert!(d.delete_forward());
        assert!(d.delete_forward());
        assert_eq!(d.text(), "de");
        d.undo();
        assert_eq!(d.text(), "abcde");
    }

    #[test]
    fn caret_restored_on_undo_and_redo() {
        let mut d = doc("");
        d.insert("hello");
        assert_eq!(d.caret(), 5);
        d.set_caret(1);
        d.insert("X"); // groupe 2
        assert_eq!(d.text(), "hXello");
        d.undo(); // restaure le caret avant la frappe de « X » : position 1
        assert_eq!(d.text(), "hello");
        assert_eq!(d.caret(), 1);
        d.undo(); // restaure le caret avant « hello » : début du document
        assert_eq!(d.text(), "");
        assert_eq!(d.caret(), 0);
        d.redo();
        assert_eq!(d.text(), "hello");
        assert_eq!(d.caret(), 5); // fin du groupe « hello »
    }

    #[test]
    fn new_edit_after_undo_kills_redo() {
        let mut d = doc("");
        d.insert("aaa");
        d.undo();
        assert!(d.can_redo());
        d.insert("b"); // la chronologie diverge
        assert!(!d.can_redo());
        assert_eq!(d.text(), "b");
        d.undo();
        assert_eq!(d.text(), "");
    }

    #[test]
    fn caret_move_does_not_kill_redo() {
        let mut d = doc("");
        d.insert("abc");
        d.undo();
        d.set_caret(0); // déplacer le caret ≠ éditer : redo intact
        assert!(d.can_redo());
        d.redo();
        assert_eq!(d.text(), "abc");
    }

    #[test]
    fn replace_range_on_empty_document() {
        let mut d = doc("the quick brown fox");
        d.set_caret(4);
        d.replace_range(4..9, "slow");
        assert_eq!(d.text(), "the slow brown fox");
        assert_eq!(d.caret(), 8); // 4 + "slow".chars()
        d.undo();
        assert_eq!(d.text(), "the quick brown fox");
        d.redo();
        assert_eq!(d.text(), "the slow brown fox");
    }

    #[test]
    fn unicode_roundtrip_undo() {
        // 🎉 compte pour 1 caractère, pas 4 octets : les positions restent
        // cohérentes entre l'édition et l'annulation.
        let mut d = doc("");
        d.set_caret(0);
        d.insert("🎉");
        d.insert(" rust");
        assert_eq!(d.text(), "🎉 rust");
        d.undo();
        assert_eq!(d.text(), "");
        d.redo();
        assert_eq!(d.text(), "🎉 rust");
        // suppressions : "rust " d'abord ("rust"+espace = Retour arrière×5)
        for _ in 0..5 {
            d.delete_backward();
        }
        assert_eq!(d.text(), "🎉");
        d.undo();
        assert_eq!(d.text(), "🎉 rust");
    }

    #[test]
    fn clean_tracking_via_revision() {
        let mut d = doc("hello");
        d.mark_saved();
        assert!(d.is_clean());
        d.insert("!");
        assert!(!d.is_clean());
        d.undo(); // revient exactement à l'état enregistré : re-propre
        assert!(d.is_clean());
        d.redo();
        assert!(!d.is_clean());
        d.mark_saved();
        assert!(d.is_clean());
    }

    #[test]
    fn fresh_document_is_dirty() {
        // Aucun point d'enregistrement : non propre, même vide
        assert!(!Document::new().is_clean());
    }

    #[test]
    fn group_cap_bounds_a_single_undo() {
        // Au-delà de la borne, la frappe continue éclate en plusieurs groupes
        let mut d = doc("");
        let many = "x".repeat(DEFAULT_GROUP_CAP + 10);
        for c in many.chars() {
            let mut s = String::new();
            s.push(c);
            d.insert(&s);
        }
        assert_eq!(d.text().len(), DEFAULT_GROUP_CAP + 10);
        d.undo();
        // le 1er groupe annulé est le plus récent : les 10 caractères excédentaires
        assert_eq!(d.text().len(), DEFAULT_GROUP_CAP);
        d.undo();
        assert!(d.text().is_empty()); // puis les 512 du gros groupe
        assert!(!d.can_undo());
    }

    #[test]
    fn undo_redo_on_empty_history_is_noop() {
        let mut d = doc("fixed");
        d.undo();
        d.redo();
        assert_eq!(d.text(), "fixed");
    }

    #[test]
    fn delete_backward_at_start_and_forward_at_end_do_nothing() {
        let mut d = doc("ab");
        assert!(!d.delete_backward()); // canon : mute
        d.set_caret(0);
        assert!(!d.delete_backward());
        d.set_caret(2);
        assert!(!d.delete_forward());
        assert_eq!(d.text(), "ab");
        assert!(!d.can_undo());
    }

    #[test]
    fn mixed_edits_make_separate_groups() {
        // frappe, puis remplacement (sélection écrasée) : 2 groupes
        let mut d = doc("");
        d.insert("abc");
        d.set_caret(0);
        d.replace_range(0..1, "X"); // remplace 'a' : pas une fusion
        assert_eq!(d.text(), "Xbc");
        d.undo();
        assert_eq!(d.text(), "abc");
        d.undo();
        assert_eq!(d.text(), "");
    }

    #[test]
    fn from_bytes_and_save_roundtrip() {
        // UTF-16 LE avec BOM : détecté, décodé, conservé à la sauvegarde
        let bytes = encoding::encode("héllo wörld", Encoding::Utf16Le);
        let d = Document::from_bytes(&bytes);
        assert_eq!(d.encoding(), Encoding::Utf16Le);
        assert_eq!(d.text(), "héllo wörld");
        assert_eq!(d.save_bytes(), bytes);

        // Windows-1252 : idem
        let cp = encoding::encode("“café”", Encoding::Windows1252);
        let d = Document::from_bytes(&cp);
        assert_eq!(d.encoding(), Encoding::Windows1252);
        assert_eq!(d.text(), "“café”");
        assert_eq!(d.save_bytes(), cp);

        // UTF-8 pur : pas de BOM ajouté
        let d = Document::from_bytes(b"plain text");
        assert_eq!(d.encoding(), Encoding::Utf8);
        assert_eq!(d.save_bytes(), b"plain text");
    }
}