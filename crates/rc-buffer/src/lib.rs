//! rc-buffer — cœur d'édition, port modernisé de `ng::buffer_t` de TextMate.
//!
//! TextMate reposait sur `ng::detail::storage_t` (chunks partageables) et
//! `oak::basic_tree_t` (AA-tree + index de Fenwick) pour obtenir :
//!
//! * insertion/suppression mieux que O(N)
//! * conversion position ↔ ligne en O(log N)
//!
//! **Modernisation** : ces structures sont remplacées par [`ropey::Rope`],
//! un rope B-tree éprouvé qui garantit exactement ces complexités
//! (O(log N) pour tout), avec un support Unicode natif (indexation en
//! `char`, pas en octets). Le compteur de [`Buffer::revision`] reproduit
//! la garde `bufferRev == revision()` de TextMate : le parsing asynchrone
//! compare la révision au départ et à l'arrivée pour jeter les résultats
//! obsolètes.

use std::ops::Range;
use std::path::Path;

use ropey::Rope;

/// Un tampon de texte éditable, optimisé pour l'édition interactive.
#[derive(Debug, Clone, Default)]
pub struct Buffer {
    rope: Rope,
    revision: u64,
}

impl Buffer {
    /// Un buffer vide.
    pub fn new() -> Self {
        Self { rope: Rope::new(), revision: 0 }
    }

    /// Construit un buffer depuis une chaîne.
    pub fn from_str(text: &str) -> Self {
        Self { rope: Rope::from_str(text), revision: 0 }
    }

    /// Construit un buffer depuis un fichier (encodage UTF-8).
    pub fn from_file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        Ok(Self { rope: Rope::from_reader(std::fs::File::open(path)?)?, revision: 0 })
    }

    /// Écrit le contenu du buffer dans un fichier.
    pub fn to_file(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        self.rope.write_to(std::fs::File::create(path)?)
    }

    /// Le nombre de caractères (pas d'octets !) du buffer.
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Le buffer est-il vide ?
    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    /// Le contenu complet, sous forme de `String`.
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Insère `text` à la position `char_idx` (en caractères).
    ///
    /// Panique si `char_idx` dépasse la fin du buffer.
    pub fn insert(&mut self, char_idx: usize, text: &str) {
        assert!(char_idx <= self.len_chars(), "insert position out of bounds");
        self.rope.insert(char_idx, text);
        self.revision += 1;
    }

    /// Supprime la plage `range` (en caractères).
    pub fn remove(&mut self, range: Range<usize>) {
        assert!(range.end <= self.len_chars(), "remove range out of bounds");
        self.rope.remove(range.start..range.end);
        self.revision += 1;
    }

    /// Remplace la plage `range` par `text` (opération atomique).
    pub fn replace(&mut self, range: Range<usize>, text: &str) {
        let start = range.start;
        if range.is_empty() {
            self.insert(start, text);
        } else {
            self.remove(range);
            if !text.is_empty() {
                self.insert(start, text);
            }
        }
    }

    // =====================
    // = Navigation lignes =
    // =====================

    /// Le nombre de lignes. Un buffer vide compte 1 ligne (comme TextMate).
    pub fn line_count(&self) -> usize {
        self.rope.len_lines().max(1)
    }

    /// La ligne `line_idx` (0-based), sans sa fin de ligne.
    pub fn line(&self, line_idx: usize) -> String {
        let line = self.rope.line(line_idx);
        let mut s = line.to_string();
        while s.ends_with('\n') || s.ends_with('\r') {
            s.pop();
        }
        s
    }

    /// Position en caractères du début de la ligne `line_idx`.
    pub fn line_to_char(&self, line_idx: usize) -> usize {
        self.rope.line_to_char(line_idx)
    }

    /// Numéro de ligne contenant la position `char_idx`.
    pub fn char_to_line(&self, char_idx: usize) -> usize {
        self.rope.char_to_line(char_idx)
    }

    // =====================
    // = Suivi de révision =
    // =====================

    /// Le numéro de révision : incrémenté à **chaque** mutation.
    ///
    /// C'est l'équivalent de `buffer_t::revision()` dans TextMate — le
    /// mécanisme qui permet au parsing asynchrone de détecter que le buffer
    /// a changé pendant qu'il travaillait, et de jeter son résultat.
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

impl From<&str> for Buffer {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

impl From<String> for Buffer {
    fn from(s: String) -> Self {
        Self::from_str(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer() {
        let b = Buffer::new();
        assert_eq!(b.len_chars(), 0);
        assert_eq!(b.line_count(), 1);
        assert_eq!(b.text(), "");
        assert_eq!(b.revision(), 0);
    }

    #[test]
    fn insert_and_remove() {
        let mut b = Buffer::new();
        b.insert(0, "hello world");
        assert_eq!(b.text(), "hello world");
        assert_eq!(b.revision(), 1);

        b.remove(5..11);
        assert_eq!(b.text(), "hello");
        assert_eq!(b.revision(), 2);

        b.insert(5, ", TextMate!");
        assert_eq!(b.text(), "hello, TextMate!");
        assert_eq!(b.revision(), 3);
    }

    #[test]
    fn replace_is_atomic_semantically() {
        let mut b = Buffer::from_str("fn main() {}");
        b.replace(3..7, "toString");
        assert_eq!(b.text(), "fn toString() {}");
        // replace d'une plage vide = insertion
        b.replace(0..0, "// ");
        assert_eq!(b.text(), "// fn toString() {}");
    }

    #[test]
    fn line_navigation() {
        let b = Buffer::from_str("first\nsecond\nthird");
        assert_eq!(b.line_count(), 3);
        assert_eq!(b.line(0), "first");
        assert_eq!(b.line(1), "second");
        assert_eq!(b.line(2), "third");
        assert_eq!(b.line_to_char(1), 6);
        assert_eq!(b.char_to_line(6), 1);
        assert_eq!(b.char_to_line(0), 0);
    }

    #[test]
    fn crlf_lines() {
        let b = Buffer::from_str("a\r\nb\r\n");
        assert_eq!(b.line_count(), 3);
        assert_eq!(b.line(0), "a");
        assert_eq!(b.line(1), "b");
    }

    #[test]
    fn unicode_positions() {
        // 🎉 est un seul `char` (4 octets, 2 unités UTF-16) — les index
        // du buffer sont en caractères, pas en octets ni en UTF-16.
        let mut b = Buffer::from_str("🎉 rust");
        assert_eq!(b.len_chars(), 6); // 🎉 + espace + r + u + s + t
        assert_eq!(b.text().len(), 9); // octets : 4 + 1 + 4
        b.insert(1, "🚀");
        assert_eq!(b.text(), "🎉🚀 rust");
        assert_eq!(b.len_chars(), 7);
        assert_eq!(b.char_to_line(4), 0);
    }

    #[test]
    fn unicode_line_navigation() {
        let b = Buffer::from_str("héllo wörld\n中文文本");
        assert_eq!(b.line(0), "héllo wörld");
        assert_eq!(b.line(1), "中文文本");
        assert_eq!(b.line_to_char(1), 12);
    }

    #[test]
    fn replace_preserves_rest() {
        let mut b = Buffer::from_str("the quick brown fox");
        b.replace(4..9, "slow");
        assert_eq!(b.text(), "the slow brown fox");
    }

    #[test]
    fn large_edits_stay_fast() {
        // Sanity check : 100k insertions unitaires ne doivent pas dégrader
        // en O(N²) perceptible (ropey garde tout en O(log N)).
        let mut b = Buffer::new();
        for i in 0..100_000 {
            let line = format!("line {i}\n");
            b.insert(b.len_chars(), &line);
        }
        assert_eq!(b.line_count(), 100_001);
        assert_eq!(b.line(50_000), "line 50000");
        assert_eq!(b.revision(), 100_000);
    }

    #[test]
    fn text_roundtrip_via_string() {
        let src = "abc\ndef\nghi";
        let mut b = Buffer::from(src);
        b.insert(0, "x");
        assert_eq!(b.text(), "xabc\ndef\nghi");
    }
}
