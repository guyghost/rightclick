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
│                     rc-document (à venir)                        │
│   Modèle document · undo/redo · encodages · I/O asynchrone       │
├──────────────────────────────────────────────────────────────────┤
│                     rc-layout (à venir)                          │
│   Soft wrap · folds · rendu vers cible graphique                 │
├──────────────────────────────────────────────────────────────────┤
│              rc-parse (à venir) · rc-scope (✅ phase 1)          │
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

## État : phase 1 — socle

- [x] Workspace Cargo (4 crates)
- [x] `rc-text` : fins de ligne, `is_word_char`, unité d'indentation
- [x] `rc-buffer` : rope, navigation lignes, révisions (tests incl. 100k lignes)
- [x] `rc-scope` : scopes + sélecteurs (sémantique TextMate)
- [x] `rc-editor` : fenêtre, ouverture/enregistrement, barre d'état
- [ ] `rc-document` : undo/redo, encodages, watch filesystem
- [ ] `rc-layout` : wrap, folds, rendu custom piloté par `rc-buffer`
- [ ] `rc-parse` : grammar engine + parsing incrémental sur threads

## Lancer

```sh
cargo run -p rc-editor -- chemin/vers/fichier.txt
```

## Tester

```sh
cargo test --workspace
```

## Licence

GPL-3.0-or-later — comme TextMate.
