//! rc-parse — moteur de grammaires TextMate, port modernisé de `ng::parse`.
//!
//! C'est le cœur différenciant : il transforme un buffer de texte brut en
//! une suite de scopes pilotée par une grammaire `.tmLanguage`. Deux
//! qualités héritées de TextMate :
//!
//! * **Compatibility bundles** — le format `.tmLanguage` (plist XML ou
//!   binaire, ou JSON dans les versions récentes) est chargé tel quel, et
//!   les motifs utilisent Oniguruma via la crate `onig` (lookbehind,
//!   backreferences, `\G` …) : 15 ans de grammaires existantes s'ouvrent
//!   sans migration.
//! * **Parsing incrémental** — le parseur ne retravaille que depuis la
//!   première ligne modifiée, et s'arrête dès que la pile de scopes d'une
//!   ligne redevient celle qu'il avait calculée : tout ce qui suit est
//!   garanti inchangé (l'invariant de TextMate : même pile d'entrée →
//!   mêmes scopes de sortie).
//!
//! L'API : [`Grammar::load`] charge une grammaire, [`Parser::parse_from`]
//! reparse incrémentalement un buffer, et [`Parser::line_ranges`] expose
//! les paires (scope, plage) par ligne pour le rendu.
//!
//! Limites assumées (documentées dans chaque module) : les règles
//! `while` et les motifs à l'intérieur des `captures` ne sont pas
//! encore pris en charge ; la profondeur d'expansion des `include` est
//! bornée pour garantir la terminaison.

pub mod grammar;
pub mod parser;

pub use grammar::{Captures, Grammar, GrammarError, Rule, RuleKind};
pub use parser::{Node, Parser, ScopedRange};