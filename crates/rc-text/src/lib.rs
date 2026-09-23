//! rc-text — primitives texte, port modernisé du framework `text/` de TextMate.
//!
//! Détection des fins de ligne, classification de caractères et indentation.
//! Aucune dépendance : ce crate est la fondation de tout le reste.

/// Style de fin de ligne utilisé par un document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Newline {
    /// `\r\n` — Windows / CRLF
    Crlf,
    /// `\r` — ancien Mac / CR
    Cr,
    /// `\n` — Unix / LF
    #[default]
    Lf,
    /// Aucune fin de ligne trouvée (document vide ou mono-ligne)
    None,
}

impl Newline {
    /// Représentation textuelle de cette fin de ligne.
    pub fn as_str(self) -> &'static str {
        match self {
            Newline::Crlf => "\r\n",
            Newline::Cr => "\r",
            Newline::Lf | Newline::None => "\n",
        }
    }

    /// Détecte le style de fin de ligne dominant d'un texte.
    ///
    /// Comme TextMate : la première fin de ligne rencontrée fait foi.
    pub fn detect(text: &str) -> Self {
        match text.find(['\r', '\n']) {
            Some(i) => match (text.as_bytes()[i], text.as_bytes().get(i + 1)) {
                (b'\r', Some(b'\n')) => Newline::Crlf,
                (b'\r', _) => Newline::Cr,
                _ => Newline::Lf,
            },
            None => Newline::None,
        }
    }
}

/// Un caractère fait-il partie d'un « mot » ?
///
/// Utilisé pour le double-clic, la navigation par mot (⌥←/⌥→) et le classement
/// des symboles — port de `text::is_word_char` de TextMate.
pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Unité d'indentation détectée d'un document — port minimal de `text/indent.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentUnit {
    /// Tabulations
    Tabs,
    /// N espaces
    Spaces(usize),
}

impl IndentUnit {
    /// Détecte l'unité d'indentation : le motif (tab vs nombre d'espaces)
    /// le plus fréquent en début de ligne l'emporte.
    pub fn detect(text: &str) -> Self {
        let mut tab_lines = 0usize;
        let mut two = 0usize;
        let mut four = 0usize;

        for line in text.lines().skip_while(|l| l.is_empty()) {
            if line.starts_with('\t') {
                tab_lines += 1;
            } else if line.starts_with("    ") && !line.starts_with("     ") {
                four += 1;
            } else if line.starts_with("  ") && !line.starts_with("   ") {
                two += 1;
            }
        }

        if tab_lines >= two && tab_lines >= four {
            IndentUnit::Tabs
        } else if two >= four {
            IndentUnit::Spaces(2)
        } else {
            IndentUnit::Spaces(4)
        }
    }

    /// La chaîne correspondant à une indentation de niveau `level`.
    pub fn indent_str(self, level: usize) -> String {
        match self {
            IndentUnit::Tabs => "\t".repeat(level),
            IndentUnit::Spaces(n) => " ".repeat(n * level),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_newline_styles() {
        assert_eq!(Newline::detect("a\nb\n"), Newline::Lf);
        assert_eq!(Newline::detect("a\r\nb\r\n"), Newline::Crlf);
        assert_eq!(Newline::detect("a\rb\r"), Newline::Cr);
        assert_eq!(Newline::detect("single line"), Newline::None);
        assert_eq!(Newline::detect(""), Newline::None);
        // La première fin de ligne fait foi
        assert_eq!(Newline::detect("a\r\nb\n"), Newline::Crlf);
    }

    #[test]
    fn newline_roundtrip() {
        for nl in [Newline::Lf, Newline::Crlf, Newline::Cr] {
            assert_eq!(Newline::detect(nl.as_str()), nl);
        }
    }

    #[test]
    fn word_chars() {
        assert!(is_word_char('a'));
        assert!(is_word_char('É'));
        assert!(is_word_char('_'));
        assert!(is_word_char('3'));
        assert!(!is_word_char(' '));
        assert!(!is_word_char('-'));
        assert!(!is_word_char('\t'));
    }

    #[test]
    fn detect_indent() {
        assert_eq!(IndentUnit::detect("\tfn a() {}\n\tfn b() {}"), IndentUnit::Tabs);
        assert_eq!(
            IndentUnit::detect("fn a() {\n    return;\n}"),
            IndentUnit::Spaces(4)
        );
        assert_eq!(
            IndentUnit::detect("fn a() {\n  return;\n}"),
            IndentUnit::Spaces(2)
        );
    }

    #[test]
    fn indent_str_levels() {
        assert_eq!(IndentUnit::Tabs.indent_str(2), "\t\t");
        assert_eq!(IndentUnit::Spaces(4).indent_str(2), "        ");
    }
}
