//! rc-scope — port modernisé du framework `scope/` de TextMate.
//!
//! Un *scope* décrit « où l'on se trouve » dans un document : une pile de
//! chemins hiérarchiques. `source.rust string.quoted.double` se lit :
//! du Rust, puis dans une chaîne entre guillemets doubles.
//!
//! Les thèmes et snippets ciblent les scopes via des *sélecteurs* :
//! chaque groupe du sélecteur (`string.quoted`) doit être le préfixe
//! d'au moins un chemin de la pile — sémantique TextMate conservée,
//! format `.tmLanguage` compatible.

/// Un scope : une pile de chemins hiérarchiques.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Scope {
    paths: Vec<Vec<String>>,
}

impl Scope {
    /// Construit un scope depuis sa représentation textuelle.
    ///
    /// `"source.rust string.quoted.double"` → pile
    /// `[[source, rust], [string, quoted, double]]`.
    pub fn parse(text: &str) -> Self {
        Scope {
            paths: text
                .split_whitespace()
                .map(|group| {
                    group.split('.').filter(|a| !a.is_empty()).map(str::to_owned).collect()
                })
                .filter(|group: &Vec<String>| !group.is_empty())
                .collect(),
        }
    }

    /// Les chemins de la pile, de la racine vers la feuille.
    pub fn paths(&self) -> &[Vec<String>] {
        &self.paths
    }

    /// Ce scope correspond-il au sélecteur `selector` ?
    ///
    /// Sémantique TextMate : **chaque** groupe du sélecteur doit être le
    /// préfixe d'un chemin de la pile. Le sélecteur `string.quoted`
    /// correspond à `string.quoted.double` mais pas à `quoted` seul
    /// (les atomes ne sautent pas de niveau dans un chemin).
    pub fn does_match(&self, selector: &Scope) -> bool {
        selector
            .paths
            .iter()
            .all(|sel_path| self.paths.iter().any(|path| is_prefix(sel_path, path)))
    }

    /// La longueur du match (total d'atomes du sélecteur) — utilisée pour
    /// départager plusieurs thèmes qui matchent (`rank` de TextMate).
    pub fn match_len(&self, selector: &Scope) -> usize {
        if self.does_match(selector) {
            selector.paths.iter().map(|p| p.len()).sum()
        } else {
            0
        }
    }
}

/// `prefix` est-il le préfixe de `path` ?
fn is_prefix(prefix: &[String], path: &[String]) -> bool {
    prefix.len() <= path.len() && prefix.iter().zip(path).all(|(a, b)| a == b)
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, path) in self.paths.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{}", path.join("."))?;
        }
        Ok(())
    }
}

/// Un sélecteur est simplement un [`Scope`] utilisé en requête.
pub fn selector(text: &str) -> Scope {
    Scope::parse(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scope() {
        let s = Scope::parse("source.rust string.quoted.double");
        assert_eq!(s.paths().len(), 2);
        assert_eq!(s.paths()[0], vec!["source", "rust"]);
        assert_eq!(s.paths()[1], vec!["string", "quoted", "double"]);
    }

    #[test]
    fn parse_ignores_extra_whitespace() {
        let s = Scope::parse("  source.rust   string  ");
        assert_eq!(s.paths().len(), 2);
        assert_eq!(Scope::parse("").paths().len(), 0);
    }

    #[test]
    fn selector_matching() {
        let scope = Scope::parse("source.rust string.quoted.double");

        // Préfixes valides, dans un même chemin
        assert!(scope.does_match(&selector("string")));
        assert!(scope.does_match(&selector("string.quoted")));
        assert!(scope.does_match(&selector("source")));
        assert!(scope.does_match(&selector("source.rust")));
        assert!(scope.does_match(&selector("")));

        // Pas de saut de niveau, pas d'atome isolé
        assert!(!scope.does_match(&selector("source.python")));
        assert!(!scope.does_match(&selector("quoted")));
        assert!(!scope.does_match(&selector("string.quoted.single")));
        assert!(!scope.does_match(&selector("rust string")));
    }

    #[test]
    fn match_len_ranks_best_selector() {
        let scope = Scope::parse("source.rust string.quoted.double");
        let short = selector("string");
        let long = selector("string.quoted.double");

        assert_eq!(scope.match_len(&short), 1);
        assert_eq!(scope.match_len(&long), 3);
        // Le sélecteur le plus long (plus spécifique) gagne
        assert!(scope.match_len(&long) > scope.match_len(&short));
        assert_eq!(scope.match_len(&selector("quoted")), 0);
    }

    #[test]
    fn display_roundtrip() {
        for text in ["source.rust", "source.rust string.quoted.double", "text"] {
            assert_eq!(Scope::parse(text).to_string(), text);
        }
    }
}
