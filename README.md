# RightClick

**Une modernisation en profondeur de TextMate, en Rust.**

RightClick reprend les principes d'architecture qui ont fait la longévité de
TextMate 2 (cœur d'édition sans GUI, parsing incrémental asynchrone, système
de scopes/bundles) et les réimplémente avec les outils modernes de l'écosystème
Rust.

## Architecture

L'équivalent Rust de la stratification TextMate :

```
┌──────────────────────────────────────────────────────────────────┐
│                    rc-editor (coquille GUI, iced)                │
│   Surface de frappe · dialogues · barre d'état · thème           │
├──────────────────────────────────────────────────────────────────┤
│                     rc-document (✅ phase 2a)                    │
│   Modèle document · caret · undo/redo · encodages               │
├──────────────────────────────────────────────────────────────────┤
│                     rc-layout (à venir)                          │
│   Soft wrap · folds · rendu vers cible graphique                 │
├──────────────────────────────────────────────────────────────────┤
│              rc-parse (✅ phase 2b) · rc-scope (✅ phase 1)    │
│   Grammar engine · selectors · scopes (compat .tmLanguage)       │
├──────────────────────────────────────────────────────────────────┤
│                       rc-buffer (✅ phase 1)                     │
│   Rope (ropey) · index de lignes · compteur de révision          │
│   ≡ TextMate : ng::storage_t + oak::basic_tree_t + ng::buffer_t  │
├──────────────────────────────────────────────────────────────────┤
│                       rc-text (✅ phase 1)                       │
│   Fins de ligne · classification · indentation                   │
│   ≡ TextMate : Frameworks/text/                                  │
└──────────────────────────────────────────────────────────────────┘
```

### Choix de modernisation

| TextMate 2 (C++/Obj-C) | RightClick (Rust) | Pourquoi |
|---|---|---|
| `ng::detail::storage_t` (chunks) + `oak::basic_tree_t` (AA-tree + Fenwick) | `ropey::Rope` (rope B-tree) | Mêmes garanties O(log N), bibliothèque éprouvée au lieu de structures maison |
| Index en octets/UTF-8 manuel | Indexation native en `char` | Unicode first |
| `buffer_t::revision()` + GCD (`dispatch_async`) | `revision()` conservé + threads/tokio (phase 2) | Le parsing asynchrone compare la révision pour jeter les résultats obsolètes |
| Scopes `.tmLanguage` (regex Onigmo) | Format conservé, moteur à revoir (phase 3) | Compatibilité avec 15 ans de bundles existants |
| Build `rave` → ninja | Cargo workspace | Standard Rust |
| Objective-C++ GUI (AppKit) | iced (GPU, cross-platform) | Remplaçable : le cœur n'en dépend pas |

### Complexités garanties (cœur d'édition)

* `insert` / `remove` / `replace` : **O(log N)**
* `line(idx)` / `char_to_line` / `line_to_char` : **O(log N)**
* Révision incrémentée à chaque mutation → invalidation propre du parsing asynchrone

## État : phase 2b — grammaires

- [x] Workspace Cargo (6 crates)
- [x] `rc-text` : fins de ligne, `is_word_char`, unité d'indentation
- [x] `rc-buffer` : rope, navigation lignes, révisions (tests incl. 100k lignes)
- [x] `rc-scope` : scopes + sélecteurs (sémantique TextMate)
- [x] `rc-document` : caret, undo/redo par records coalescés, propreté,
      encodages (BOM UTF-8/UTF-16, UTF-16 sans BOM, Windows-1252)
- [x] `rc-parse` : chargement `.tmLanguage` (plist XML/binaire + JSON),
      moteur oniguruma (`\G` ancré), begin/end multi-ligne, captures,
      repository/`$self`, parsing incrémental (arrêt sur pile stable)
- [x] `rc-editor` : fenêtre, ouverture/enregistrement, barre d'état
      (détection d'encodage à l'ouverture)
- [ ] `rc-document` : watch filesystem, normalisation fins de ligne à l'enregistrement
- [ ] `rc-parse` : règles `while`, parsing asynchrone sur threads
- [ ] `rc-layout` : wrap, folds, rendu custom piloté par `rc-buffer`
      (coloration par scopes via rc-parse)

## Démo

```sh
cargo run -p rc-parse --example highlight -- chemin/vers/fichier.txt
```

Le moteur de grammaires est visible dans le terminal : commentaires,
chaînes (avec échappements), mots-clés, nombres — colorisés depuis les
scopes émis par `rc-parse` (grammaire embarquée `source.rcd`).

## Tester

```sh
cargo test --workspace
```

## Licence

GPL-3.0-or-later — comme TextMate.
