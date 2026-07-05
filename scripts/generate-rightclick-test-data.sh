#!/usr/bin/env bash
set -euo pipefail
# scripts/generate-rightclick-test-data.sh
# Production-scale LOCAL synthetic test data for RightClick.
#
# SAFETY CONTRACT (MANDATORY):
# - This script ONLY ever removes directories that contain a .rightclick-test-marker
#   file with the magic prefix it created itself.
# - It performs double-checks before any rm -rf.
# - It will NEVER touch $HOME, the source checkout, or any unmarked directory.
# - All data is synthetic and isolated.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

MARKER_MAGIC="rightclick-synthetic-test-data v1"

log() { printf '==> %s\n' "$*" >&2; }
die() { echo "ERROR: $*" >&2; exit 1; }

create_marker() {
  local dir="$1"
  local ts
  ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  printf '%s %s %s\n' "$MARKER_MAGIC" "$ts" "$dir" > "$dir/.rightclick-test-marker"
}

is_our_dir() {
  local dir="$1"
  [[ -f "$dir/.rightclick-test-marker" ]] && grep -q "^$MARKER_MAGIC" "$dir/.rightclick-test-marker"
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "Required command not found: $1"
}

encode_path() {
  python3 -c '
import sys
p = sys.argv[1]
print(p.replace("/", "-").replace(".", "-").replace("_", "-"))
' "$1"
}

md5_str() {
  python3 -c '
import hashlib, sys
print(hashlib.md5(sys.argv[1].encode("utf-8")).hexdigest())
' "$1"
}

init_git_repo() {
  local repo="$1"
  mkdir -p "$repo"
  cd "$repo"

  git init -q
  git config user.name "RightClick Test Data"
  git config user.email "test@rightclick.local"

  mkdir -p src/{core,adapters,plugins,ui} docs tests .rightclick/{intents,logs}

  cat > README.md <<'EOT'
# rightclick-test-project

Synthetic project for RightClick E2E testing.
EOT

  cat > Cargo.toml <<'EOT'
[package]
name = "rightclick-test-project"
version = "0.1.0"
edition = "2021"
EOT

  echo "initial" > src/main.rs
  echo "lib" > src/lib.rs
  echo "# placeholder" > docs/ARCH.md
  echo "placeholder" > tests/smoke.rs

  GIT_AUTHOR_DATE="2026-01-01T10:00:00Z" GIT_COMMITTER_DATE="2026-01-01T10:00:00Z" \
    git add -A
  GIT_AUTHOR_DATE="2026-01-01T10:00:00Z" GIT_COMMITTER_DATE="2026-01-01T10:00:00Z" \
    git commit -q -m "chore: initial commit"

  for i in $(seq 1 55); do
    echo "// change $i" > "src/file_${i}.rs"
    git add "src/file_${i}.rs"
    local msg
    case $((i % 7)) in
      0) msg="feat: add module file_${i}" ;;
      1) msg="fix: handle edge case in file_${i}" ;;
      2) msg="refactor: simplify file_${i}" ;;
      3) msg="docs: update file_${i}" ;;
      4) msg="test: add coverage for file_${i}" ;;
      5) msg="perf: optimize hot path in file_${i}" ;;
      *) msg="chore: tweak file_${i}" ;;
    esac
    local day=$(( (i / 3) + 1 ))
    local ts="2026-01-$(printf '%02d' $day)T1$(printf '%02d' $((i%10))):00:00Z"
    GIT_AUTHOR_DATE="$ts" GIT_COMMITTER_DATE="$ts" git commit -q -m "$msg"
  done

  git checkout -q -b feature/ui-polish
  echo "// polish" > src/ui/polish.rs
  git add src/ui/polish.rs
  GIT_AUTHOR_DATE="2026-02-10T12:00:00Z" GIT_COMMITTER_DATE="2026-02-10T12:00:00Z" \
    git commit -q -m "feat(ui): polish sidebar and tabs"
  git checkout -q main
  git merge --no-ff feature/ui-polish -q -m "Merge pull request #42 from feature/ui-polish" || true

  git checkout -q -b feature/metrics
  echo "// metrics" > src/core/metrics.rs
  git add src/core/metrics.rs
  GIT_AUTHOR_DATE="2026-02-12T09:30:00Z" GIT_COMMITTER_DATE="2026-02-12T09:30:00Z" \
    git commit -q -m "feat: add basic metrics collection"
  git checkout -q main

  echo "dirty change" >> src/main.rs
  git stash push -q -m "WIP: experiment with layout"
  echo "spike content" > src/spike.txt
  git add src/spike.txt
  git stash push -q -m "WIP: spike new adapter"

  git checkout -q -b feature/conflict-demo
  echo "line A from feature" > src/conflict.rs
  git add src/conflict.rs
  GIT_AUTHOR_DATE="2026-02-15T14:00:00Z" GIT_COMMITTER_DATE="2026-02-15T14:00:00Z" \
    git commit -q -m "feat: conflict demo on feature"
  git checkout -q main
  echo "line A from main" > src/conflict.rs
  git add src/conflict.rs
  GIT_AUTHOR_DATE="2026-02-15T15:00:00Z" GIT_COMMITTER_DATE="2026-02-15T15:00:00Z" \
    git commit -q -m "feat: conflict demo on main"
  git merge --no-commit feature/conflict-demo || true

  echo "staged content" > src/staged.txt
  git add src/staged.txt
  echo "unstaged edit" >> src/main.rs
  echo "untracked.log" > untracked.log

  git worktree add -q -b feature/wt-a ../repo-wt-a || true
  git worktree add -q -b feature/wt-b ../repo-wt-b || true

  mkdir -p .rightclick/intents
  local now="2026-06-28T02:00:00.000000+00:00"

  cat > .rightclick/intents/implement-dark-mode.md <<EOT
---
id: intent-$(python3 -c 'import uuid; print(uuid.uuid4())' 2>/dev/null || echo "d1a2b3c4-e5f6-7890-abcd-ef1234567890")
status: draft
created: $now
updated: $now
workers: []
---

# Implement dark mode

## Description
Add a complete dark theme toggle with persistence.

## Acceptance Criteria
- [ ] Theme switches without restart
- [ ] Persists choice across sessions
- [ ] High contrast meets a11y
EOT

  cat > .rightclick/intents/fix-conversation-scroll.md <<EOT
---
id: intent-$(python3 -c 'import uuid; print(uuid.uuid4())' 2>/dev/null || echo "aabbccdd-1122-3344-5566-778899aabbcc")
status: ready
created: $now
updated: $now
workers: []
---

# Fix conversation scroll jank

## Description
Virtualize list and reduce re-renders.

## Acceptance Criteria
- [x] 60fps on 5k messages
- [ ] No layout shift on stream
EOT

  cat > .rightclick/intents/add-safe-test-data.md <<EOT
---
id: intent-$(python3 -c 'import uuid; print(uuid.uuid4())' 2>/dev/null || echo "11223344-5566-7788-99aa-bbccddeeff00")
status: in_progress
created: $now
updated: $now
workers: []
---

# Add production-scale local test data

## Description
Design and generate isolated synthetic fixtures for realistic testing.

## Acceptance Criteria
- [ ] Script is idempotent and safe (only cleans its own dirs)
- [ ] Covers all 4 adapters + rich git state + intents + worktrees
- [ ] Documented harness invocation
EOT

  cat > .rightclick/intents/refactor-adapter-detection.md <<EOT
---
id: intent-$(python3 -c 'import uuid; print(uuid.uuid4())' 2>/dev/null || echo "99887766-5544-3322-1100-aabbccddeeff")
status: draft
created: $now
updated: $now
workers: []
---

# Refactor adapter detection

## Acceptance Criteria
- [ ] Clearer error surfacing
- [ ] Better logging of encode/hash steps
EOT

  cat > .rightclick/intents/resolve-conflicts-ui.md <<EOT
---
id: intent-$(python3 -c 'import uuid; print(uuid.uuid4())' 2>/dev/null || echo "deadbeef-cafe-babe-0000-111122223333")
status: draft
created: $now
updated: $now
workers: []
---

# Resolve conflicts UI

## Acceptance Criteria
- [ ] Show conflicted files prominently
- [ ] Offer resolution actions
EOT

  cd - >/dev/null
}

create_adapter_data() {
  local fake_home="$1"
  local repo_abs="$2"
  local canon
  canon=$(cd "$repo_abs" && pwd -P)

  mkdir -p "$fake_home"

  # Claude Code (8 sessions)
  local enc
  enc=$(encode_path "$canon")
  local claude_proj="$fake_home/.claude/projects/$enc"
  mkdir -p "$claude_proj/sessions"
  for i in $(seq 1 8); do
    local sid="sess-claude-$(printf '%04d' $i)"
    local sdir="$claude_proj/sessions/$sid"
    mkdir -p "$sdir"
    local ts=$((1700000000 + i*3600))
    cat > "$sdir/metadata.json" <<EOT
{"name":"Claude session $i","created_at":$ts,"updated_at":$((ts+1200))}
EOT
    {
      echo "{\"role\":\"user\",\"content\":\"Implement feature $i\",\"timestamp\":$ts}"
      echo "{\"role\":\"assistant\",\"content_blocks\":[{\"type\":\"text\",\"text\":\"Working on $i\"},{\"type\":\"tool_use\",\"id\":\"tu$i\",\"name\":\"Edit\",\"input\":{\"file\":\"src/a.rs\"}}],\"timestamp\":$((ts+30)),\"usage\":{\"input_tokens\":120,\"output_tokens\":340},\"model\":\"claude-3-5-sonnet\"}"
      echo "{\"role\":\"user\",\"content\":\"Looks good, add tests\",\"timestamp\":$((ts+90))}"
    } > "$sdir/conversation.jsonl"
  done

  # Cursor (8 conversations)
  local md
  md=$(md5_str "$canon")
  local cursor_dir="$fake_home/.cursor/chats/$md"
  mkdir -p "$cursor_dir"
  local db="$cursor_dir/store.db"
  python3 - "$db" <<'PY'
import sqlite3, sys, json
db = sys.argv[1]
con = sqlite3.connect(db)
con.executescript("""
CREATE TABLE IF NOT EXISTS conversations (id TEXT PRIMARY KEY, title TEXT, created_at INTEGER, updated_at INTEGER);
CREATE TABLE IF NOT EXISTS messages (id TEXT PRIMARY KEY, conversation_id TEXT, role TEXT, content TEXT, created_at INTEGER, model TEXT, metadata TEXT);
""")
for i in range(1,9):
    cid = f"cursor-conv-{i}"
    cts = 1701000000 + i*1000
    con.execute("INSERT OR IGNORE INTO conversations (id,title,created_at,updated_at) VALUES (?,?,?,?)", (cid, f"Cursor conv {i}", cts, cts+500))
    con.execute("INSERT OR IGNORE INTO messages (id,conversation_id,role,content,created_at,model,metadata) VALUES (?,?,?,?,?,?,?)",
                (f"m{i}a", cid, "user", "Do X", cts, None, None))
    meta = json.dumps({"usage":{"prompt_tokens":80,"completion_tokens":220}})
    con.execute("INSERT OR IGNORE INTO messages (id,conversation_id,role,content,created_at,model,metadata) VALUES (?,?,?,?,?,?,?)",
                (f"m{i}b", cid, "assistant", "Done X", cts+10, "gpt-4o", meta))
con.commit()
con.close()
PY

  # Codex (8 sessions + mapping)
  local codex_root="$fake_home/.codex"
  mkdir -p "$codex_root/sessions"
  local sess_list=()
  for i in $(seq 1 8); do
    local sid="codex-sess-$(printf '%04d' $i)"
    sess_list+=("$sid")
    local sdir="$codex_root/sessions/$sid"
    mkdir -p "$sdir"
    local ts=$((1702000000 + i*900))
    cat > "$sdir/metadata.json" <<EOT
{"name":"Codex $i","created_at":$ts,"updated_at":$((ts+600)),"total_usage":{"prompt_tokens":50,"completion_tokens":180}}
EOT
    {
      echo "{\"role\":\"user\",\"content\":\"Codex task $i\",\"timestamp\":$ts}"
      echo "{\"role\":\"assistant\",\"content_blocks\":[{\"type\":\"text\",\"text\":\"Implemented $i\"}],\"timestamp\":$((ts+40)),\"usage\":{\"prompt_tokens\":30,\"completion_tokens\":90},\"model\":\"o3-mini\"}"
    } > "$sdir/conversation.jsonl"
  done
  python3 - "$codex_root/project_mappings.json" "$canon" "${sess_list[@]}" <<'PY'
import json, sys
out, canon = sys.argv[1], sys.argv[2]
sessions = sys.argv[3:]
data = {"projects": {canon: sessions}}
with open(out, "w") as f:
    json.dump(data, f, indent=2)
PY

  # OpenCode (7 sessions)
  local oc_root="$fake_home/.local/share/opencode/storage"
  mkdir -p "$oc_root/message" "$oc_root/part"
  for i in $(seq 1 7); do
    local sid="opencode-sess-$i"
    local mdir="$oc_root/message/$sid"
    mkdir -p "$mdir"
    local mid="msg-op-$i"
    local created_ms=$((1703000000000 + i*100000))
    cat > "$mdir/meta-$mid.json" <<EOT
{"id":"$mid","sessionID":"$sid","role":"user","time":{"created":$created_ms},"modelID":"local","tokens":{"input":10,"output":5}}
EOT
    local pdir="$oc_root/part/$mid"
    mkdir -p "$pdir"
    cat > "$pdir/prt_001.json" <<EOT
{"id":"prt1","type":"text","text":"OpenCode request $i","messageID":"$mid","sessionID":"$sid"}
EOT
  done
}

main() {
  local force=0
  local do_clean=0
  local do_clean_all=0

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --force) force=1 ;;
      --clean) do_clean=1 ;;
      --clean-all) do_clean_all=1 ;;
      -h|--help)
        cat <<EOF
Usage: $0 [--force] [--clean | --clean-all]

Creates or cleans a fully isolated synthetic RightClick test fixture under
/tmp/rightclick-test-data-*

  --force     Overwrite existing fixture (only if marked by this script)
  --clean     Remove the most recent fixture created by this script
  --clean-all Remove ALL /tmp/rightclick-test-data-* dirs that contain the marker

After generation, run with:
  BASE=/tmp/rightclick-test-data-...
  env HOME="\$BASE/fake_home" rightclick --project "\$BASE/repo" --debug

The script will print the exact commands at the end.
EOF
        exit 0
        ;;
      *) 
        echo "Unknown argument: $1" >&2
        exit 1
        ;;
    esac
    shift
  done

  if [[ $do_clean_all -eq 1 ]]; then
    log "Cleaning ALL safe fixtures..."
    for d in /tmp/rightclick-test-data-*; do
      if [[ -d "$d" ]] && is_our_dir "$d"; then
        log "Removing $d"
        rm -rf "$d"
      fi
    done
    log "Done."
    exit 0
  fi

  if [[ $do_clean -eq 1 ]]; then
    local latest
    latest=$(ls -1dt /tmp/rightclick-test-data-* 2>/dev/null | head -1 || true)
    if [[ -n "$latest" ]] && is_our_dir "$latest"; then
      log "Removing $latest"
      rm -rf "$latest"
      log "Clean complete."
    else
      log "No safe fixture found to clean."
    fi
    exit 0
  fi

  local ts
  ts=$(date +%Y%m%d)
  local rand
  rand=$(python3 -c 'import secrets; print(secrets.token_hex(4))')
  local base="/tmp/rightclick-test-data-${ts}-${rand}"
  local repo="$base/repo"
  local fake_home="$base/fake_home"

  if [[ -d "$base" ]]; then
    if [[ $force -eq 1 ]] && is_our_dir "$base"; then
      log "Force removing existing $base"
      rm -rf "$base"
    else
      die "Directory $base already exists. Use --force or --clean."
    fi
  fi

  require_cmd git
  require_cmd python3

  log "Creating fixture at $base"
  mkdir -p "$base"
  create_marker "$base"

  init_git_repo "$repo"

  local repo_abs
  repo_abs=$(cd "$repo" && pwd -P)

  create_adapter_data "$fake_home" "$repo_abs"

  create_marker "$base"

  log "Fixture created."

  cat <<EOT

DATA FIXTURE READY
------------------
Base:     $base
Repo:     $repo_abs
Fake HOME: $fake_home

Run (recommended from this checkout):
  env HOME="$fake_home" cargo run -- --project "$repo_abs" --debug

Or with installed binary:
  env HOME="$fake_home" rightclick --project "$repo_abs" --debug

To clean only this fixture:
  bash scripts/generate-rightclick-test-data.sh --clean

To clean all generated fixtures:
  bash scripts/generate-rightclick-test-data.sh --clean-all

IMPORTANT: The script will only delete directories containing the .rightclick-test-marker
file it created. It refuses to touch anything else.
EOT
}

main "$@"
