# Context: Fix OpenCode Adapter Message Loading

## Objective
Fix the OpenCode adapter to properly load conversation data:
1. Count messages in `sessions()` method
2. Implement `messages()` to load message content from JSON files
3. Implement `usage()` to aggregate token usage

## Constraints
- Platform: Rust (TUI application)
- Architecture: FC&IS (Functional Core & Imperative Shell)
- Async/await patterns with tokio
- Follow existing adapter patterns (see ClaudeCode adapter)

## Data Structure Discovered

### Session Storage
```
~/.local/share/opencode/storage/
├── session/              # Session metadata files (hash named)
├── session_diff/         # Session diffs (ses_<id>.json)
├── message/              # Messages per session
│   └── <session_id>/
│       └── msg_<id>.json # Message metadata
└── part/                 # Message content
    └── <message_id>/
        └── prt_<id>.json # Text content
```

### Message JSON Structure
```json
{
  "id": "msg_...",
  "sessionID": "ses_...",
  "role": "assistant|user",
  "time": {
    "created": 1767099449516,
    "completed": 1767099450259
  },
  "parentID": "msg_...",
  "modelID": "grok-code-fast-1",
  "providerID": "github-copilot",
  "mode": "codegen",
  "agent": "codegen",
  "path": {
    "cwd": "/path/to/project",
    "root": "/path/to/project"
  },
  "cost": 0,
  "tokens": {
    "input": 2041,
    "output": 28,
    "reasoning": 0,
    "cache": { "read": 32128, "write": 0 }
  },
  "finish": "tool-calls"
}
```

### Part JSON Structure
```json
{
  "id": "prt_...",
  "type": "text",
  "text": "message content here",
  "synthetic": false,
  "time": { "start": 0, "end": 0 },
  "messageID": "msg_...",
  "sessionID": "ses_..."
}
```

## Technical Decisions
| Decision | Justification |
|----------|---------------|
| Keep data_dir path | Use ~/.local/share/opencode/storage |
| Async file operations | Use tokio::fs for consistency |
| Parse timestamp from millis | OpenCode uses millisecond timestamps |
| Role mapping | "assistant" -> Assistant, "user" -> User, others -> defaults |

## Artifacts to Modify
| File | Action |
|------|--------|
| src/adapters/opencode.rs | Update sessions(), implement messages(), implement usage() |

## Inter-Agent Notes
<!-- @orchestrator -> @codegen: Focus on proper async file handling and error resilience -->
<!-- @orchestrator -> @tests: Test with realistic OpenCode data structures -->
