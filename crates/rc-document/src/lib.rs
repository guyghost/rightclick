//! rc-document — modèle document, port modernisé de `ng::document_t` de TextMate.
//!
//! TextMate séparait la saisie graphique du modèle : le document était un
//! `ng::document_t` qui **possédait** le buffer, le caret et l'historique
//! d'undo/redo. Cette stratification est ici reproduite par trois couches :
//!
//! * [`Document`] — état : buffer + caret + piles undo/redo + propreté
//! * [`document::Step`] — un groupe d'édition annulable d'un geste
//! * [`encoding`] — détection (BOM + heuristiques) et conversion
//!
//! L'undo/redo est **basé sur des records** (`range` + `removed` + `inserted`),
//! comme TextMate : chaque action d'édition produit un record décrivant la
//! transformation appliquée au buffer. Les records consécutifs compatibles
//! (frappe continue, Retour arrière maintenu) sont **coalescés** en un seul
//! groupe : un seul Ctrl+Z annule toute la phrase tapée d'un trait.
//!
//! La restauration du caret suit la sémantique TextMate : annuler replace le
//! caret au début du groupe, refaire le replace à sa fin.

pub mod document;
pub mod encoding;

pub use document::Document;
pub use encoding::Encoding;