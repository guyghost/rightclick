# Résumé: State Manager avec Autorisation et Navigation

## Problèmes Identifiés

### 1. Navigation cassée dans l'historique git
- **Symptôme**: Les touches `j/k` ne naviguent pas entre les commits en mode History
- **Cause**: La logique de navigation est dispersée et ne synchronise pas correctement l'état entre `selected_commit` et le système de focus

### 2. Pas de vérification d'autorisation
- **Symptôme**: On peut appeler `Stage` même sans fichier sélectionné
- **Cause**: Les actions sont exécutées directement sans vérifier les préconditions

### 3. Gestion du focus éparpillée
- **Symptôme**: Le focus est géré à plusieurs endroits (`FocusPane`, `FocusContext`, état du plugin)
- **Cause**: Pas de système centralisé de navigation au clavier

## Solution Proposée

### Architecture FC&IS (Functional Core & Imperative Shell)

```
src/
├── core/                      # PURE - Pas d'I/O
│   ├── models/
│   │   ├── state_machine.rs   # États, transitions
│   │   ├── navigation.rs      # Navigation directionnelle
│   │   └── action.rs          # Actions et guards
│   └── logic/
│       ├── guards.rs          # Logique d'autorisation
│       └── navigation.rs      # Calculs de navigation
│
└── shell/                     # IMPURE - I/O et effets
    └── machines/
        ├── mod.rs             # Exécuteur de state machine
        └── git_state_machine.rs # Intégration git
```

### Fichiers à Créer

| Fichier | Description | Lignes estimées |
|---------|-------------|-----------------|
| `core/models/state_machine.rs` | États (Initial, Ready, ItemSelected, Editing, Modal, Error) | ~150 |
| `core/models/navigation.rs` | Régions (Sidebar, Main), directions (Up, Down, Left, Right) | ~120 |
| `core/models/action.rs` | ActionId, GuardError, GuardResult | ~100 |
| `core/logic/guards.rs` | Vérification d'autorisation pure | ~150 |
| `core/logic/navigation.rs` | Calcul de navigation pur | ~180 |
| `shell/machines/mod.rs` | Exécuteur avec callbacks | ~250 |
| `shell/machines/git_state_machine.rs` | Intégration spécifique git | ~200 |

### Fichiers à Modifier

| Fichier | Modification |
|---------|--------------|
| `core/models/mod.rs` | Ajouter les exports |
| `core/logic/mod.rs` | Ajouter les exports |
| `shell/mod.rs` | Ajouter le module machines |
| `plugins/gitstatus/plugin.rs` | Utiliser GitStateMachine |
| `plugins/gitstatus/state.rs` | Synchroniser avec state machine |

## Fonctionnement

### 1. Guards (Autorisation)

```rust
// Pure function
pub fn check_guard(ctx: &ActionContext) -> GuardResult {
    match ctx.action {
        ActionId::Stage | ActionId::Unstage => {
            if ctx.context.selected_index.is_none() {
                return GuardResult::Denied(GuardError::NoSelection);
            }
            if ctx.context.view_mode != ViewMode::Status {
                return GuardResult::Denied(GuardError::WrongViewMode { ... });
            }
            GuardResult::Authorized
        }
        ...
    }
}
```

### 2. Navigation

```rust
// Pure function
pub fn calculate_navigation(
    direction: NavDirection,
    context: &StateContext,
) -> NavigationResult {
    match direction {
        NavDirection::Down => navigate_down(context),
        NavDirection::Up => navigate_up(context),
        ...
    }
}
```

### 3. Exécution (Shell)

```rust
impl StateMachineExecutor {
    pub fn execute_action(&self, action: ActionId) -> ActionResult {
        // 1. Check guard (pure)
        let guard_result = check_guard(&action_ctx);
        
        match guard_result {
            GuardResult::Denied(error) => ActionResult::Denied(error),
            GuardResult::Authorized => {
                // 2. Execute callback (side effect)
                if let Some(ref callback) = self.on_action {
                    callback(action);
                }
                // 3. Update state
                ActionResult::Success
            }
        }
    }
}
```

### 4. Intégration Git

```rust
impl GitStatusPlugin {
    fn handle_key(&mut self, key: &str) -> Vec<Command> {
        // Délégué au state machine
        let git_commands = self.state_machine.handle_key(key);
        
        // Convertir en Commandes
        for cmd in git_commands {
            match cmd {
                GitCommand::SelectIndex(idx) => {
                    // CORRECTION: Met à jour selected_commit
                    if self.state.view_mode == ViewMode::History {
                        self.state.selected_commit = Some(idx);
                        commands.push(Command::LoadCommitDetails(...));
                    }
                }
                GitCommand::ExecuteAction(ActionId::Stage) => {
                    // Guard déjà vérifié par state machine
                    commands.push(Command::StageFile(...));
                }
                ...
            }
        }
    }
}
```

## Tests

### Unit Tests (Core)
```rust
#[test]
fn test_stage_requires_selection() {
    let ctx = action_context(ActionId::Stage, None, ViewMode::Status);
    assert!(matches!(
        check_guard(&ctx),
        GuardResult::Denied(GuardError::NoSelection)
    ));
}

#[test]
fn test_navigate_down_selects_first() {
    let context = state_context(None, 5); // No selection, 5 items
    let result = navigate_down(&context);
    assert!(matches!(result, NavigationResult::Navigate { index: Some(0), .. }));
}
```

### E2E Tests (Fix Navigation)
```rust
#[tokio::test]
async fn test_git_history_navigation_with_j_k() {
    let mut plugin = create_test_plugin_with_commits(10).await;
    plugin.state.view_mode = ViewMode::History;
    
    // Press 'j' - should select first commit
    plugin.handle_key("j");
    assert_eq!(plugin.state.selected_commit, Some(0));
    
    // Press 'j' again - should select second commit
    plugin.handle_key("j");
    assert_eq!(plugin.state.selected_commit, Some(1));
}
```

## Commandes pour Implémenter

```bash
# 1. Créer les fichiers core
touch src/core/models/state_machine.rs
touch src/core/models/navigation.rs
touch src/core/models/action.rs
touch src/core/logic/guards.rs
touch src/core/logic/navigation.rs

# 2. Créer les fichiers shell
mkdir -p src/shell/machines
touch src/shell/machines/mod.rs
touch src/shell/machines/git_state_machine.rs

# 3. Compiler et tester
cargo build
cargo test

# 4. Lancer l'application pour tester la navigation git
RUST_LOG=debug cargo run
```

## Points Clés

1. **Séparation Core/Shell**: Core est pure (testable), Shell gère les effets
2. **Guards**: Chaque action vérifie ses préconditions avant exécution
3. **Navigation centralisée**: Une seule logique pour tout le clavier
4. **Callbacks**: Le shell notifie les changements via callbacks
5. **Synchronisation**: Le plugin garde l'état de rendu, la state machine l'état logique

## Prochaines Étapes

1. Implémenter les modèles Core
2. Implémenter la logique Core (guards + navigation)
3. Créer l'exécuteur Shell
4. Intégrer au plugin git
5. Tester la navigation j/k dans l'historique
