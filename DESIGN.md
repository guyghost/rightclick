---
name: RightClick
description: Terminal UI design system for a multi-agent developer cockpit — dark, dense, terminal-native.
colors:
  # Canonical palette = Default theme (Theme::default()). All four shipped themes
  # share this token structure; alternate themes are documented in §2.
  primary: "#7aa2f7"          # Soft blue — the single accent
  secondary: "#bb9af7"        # Soft purple — rare secondary accent
  success: "#9ece6a"          # Soft green — added / ok
  warning: "#e0af68"          # Soft amber — modified / caution
  error: "#f7768e"            # Soft red — removed / failure
  info: "#7dcfff"             # Cyan — info / untracked-alt
  background: "#1a1b26"       # Canvas — deep blue-black
  foreground: "#c0caf5"       # Ink — soft off-white
  muted: "#565f89"            # Receded text / placeholders
  border: "#414868"           # Chrome — plain box-drawing borders
  highlight: "#283457"        # Selection / active-item fill
  cursor: "#c0caf5"           # Cursor (matches ink)
  git-added: "#9ece6a"
  git-removed: "#f7768e"
  git-modified: "#e0af68"
  git-untracked: "#7aa2f7"
  status-bar-bg: "#16161e"    # Receded chrome (darker than canvas)
  status-bar-fg: "#a9b1d6"
  popup-bg: "#1a1b26"
  popup-border: "#414868"
  input-bg: "#24283b"         # Raised fill (lighter than canvas)
  input-placeholder: "#565f89"
  button-bg: "#7aa2f7"
  button-fg: "#1a1b26"
typography:
  display:
    fontFamily: "monospace, Menlo, SF Mono, Consolas"
    fontWeight: 700
  headline:
    fontFamily: "monospace, Menlo, SF Mono, Consolas"
    fontWeight: 700
  title:
    fontFamily: "monospace, Menlo, SF Mono, Consolas"
    fontWeight: 600
  body:
    fontFamily: "monospace, Menlo, SF Mono, Consolas"
    fontWeight: 400
  label:
    fontFamily: "monospace, Menlo, SF Mono, Consolas"
    fontWeight: 600
components:
  list-item-active:
    backgroundColor: "{colors.highlight}"
    textColor: "{colors.foreground}"
    padding: "0 1 cell"
  list-item-inactive:
    backgroundColor: "{colors.status-bar-bg}"
    textColor: "{colors.muted}"
    padding: "0 1 cell"
  popup:
    backgroundColor: "{colors.popup-bg}"
    textColor: "{colors.foreground}"
    padding: "1 cell"
  input:
    backgroundColor: "{colors.input-bg}"
    textColor: "{colors.foreground}"
    padding: "0 1 cell"
  status-bar:
    backgroundColor: "{colors.status-bar-bg}"
    textColor: "{colors.status-bar-fg}"
    padding: "0 1 cell"
  button-primary:
    backgroundColor: "{colors.button-bg}"
    textColor: "{colors.button-fg}"
    padding: "0 2 cells"
---

# Design System: RightClick

## 1. Overview

**Creative North Star: "The Operator's Console"**

RightClick is a terminal UI — a focused cockpit an operator sits in for hours.
Everything present, nothing competing, the pilot in control. The aesthetic is
the medium: character cells, contrast, and density, never transplanted web
effects. There is no CSS here, no drop-shadows, no blur, no gradient text. Depth
comes from tone and border, hierarchy comes from weight and position, and the
single accent arrives rarely enough that when it does, it means something.

The operator is an expert. The screen is read at a glance and trusted under
long sessions, so information density is a feature, not a problem — but density
is disciplined: one clear focus per surface, supporting context receding through
the muted token, never shouting. Motion is sparse and only ever conveys state
(a spinner, a transition), never decoration.

This system explicitly rejects noisy, glossy SaaS dashboards: no gradients, no
glassmorphism, no marketing polish. Anything that exists to look impressive
rather than to convey state or afford action is a defect. RightClick should
look like a serious instrument, not a consumer app.

**Key Characteristics:**

- **Terminal-native.** Monospace grid, box-drawing chrome, no web effects.
- **Dark, low-glare.** A deep blue-black canvas with a soft off-white ink,
  tuned for extended focus.
- **One accent, used sparingly.** Soft blue carries focus; everything else is
  neutral or semantic.
- **Flat and tonal.** No shadows; depth is conveyed by background tone and
  plain borders.
- **Semantic, never decorative.** Every color encodes state (added/removed/
  modified, success/warning/error) and is reinforced by glyph so color is
  never the sole signal.

## 2. Colors: The Console Palette

Four dark themes ship (`default`, `dracula`, `nord`, `tokyo-night`), all sharing
one token structure. The **Default** theme is canonical — the values above and
below — and is what new screens are designed against. Alternate themes are
palette swaps of the same roles, not different systems.

### Primary

- **Soft Blue** (`#7aa2f7`): the single accent. Used for focus, the primary
  action, untracked files, and key interactive emphasis. Its rarity is the
  point — it should cover a small fraction of any screen.

### Secondary

- **Soft Purple** (`#bb9af7`): a rare secondary accent for hover states and a
  second tier of emphasis. Used even more sparingly than Primary.

### Tertiary (semantic state)

- **Soft Green / Added** (`#9ece6a`): success and git additions.
- **Soft Amber / Modified** (`#e0af68`): caution and git modifications.
- **Soft Red / Removed** (`#f7768e`): errors and git deletions.
- **Cyan / Info** (`#7dcfff`): informational accent and syntax type names.

### Neutral

- **Canvas** (`#1a1b26`): the deep blue-black background. Lowest layer.
- **Ink** (`#c0caf5`): soft off-white body text and default foreground.
- **Muted** (`#565f89`): receded text, placeholders, comments. Anything that
  must remain legible but should not compete for attention.
- **Chrome** (`#414868`): plain single-line borders and popup borders.
- **Selection** (`#283457`): the active/selected item fill.
- **Recessed Chrome** (`#16161e`): status bar and sidebar backgrounds — darker
  than the canvas so chrome recedes behind content.
- **Raised Fill** (`#24283b`): input fields — lighter than the canvas so they
  read as an interactive surface without needing a border glow.

### Named Rules

**The One Console Rule.** The primary accent covers a small fraction of any
screen. If two equally-bright accents compete, the screen is no longer a
cockpit — demote one to muted or neutral.

**The Never-Color-Only Rule.** Git status and semantic state are never encoded
by color alone. Added / modified / removed / untracked are always paired with a
glyph (or position), so the screen stays readable for every form of color
vision. This is non-negotiable for accessibility.

**The Chrome Recedes Rule.** Status bars and sidebars use a background darker
than the content canvas; inputs use a background lighter than it. Depth is
tonal, never shadowed.

## 3. Typography

**Font:** monospace — whatever the operator's terminal renders (fallbacks:
Menlo, SF Mono, Consolas). RightClick does not choose a typeface; it commits to
the grid the terminal gives it.

**Character:** A single monospace family at multiple weights. Hierarchy comes
from **weight** (bold for headings and emphasis), **modifier** (dim for
muted/receded content, italic for secondary metadata), and **Unicode
box-drawing** for structure — never from pixel sizes. There is no `clamp()`, no
display-vs-body family pairing, no tracking tweaks: the cell grid is fixed, and
that constraint is the aesthetic.

### Hierarchy

- **Display** (bold): top-level surface titles and the rare large label. The
  heaviest weight on screen; used at most once per surface.
- **Headline** (bold): section and pane titles within a surface.
- **Title** (semibold): list headings, dialog titles, column headers.
- **Body** (regular): the default. Lists, file names, conversation lines,
  diff content. Line length is governed by pane width, not a measure.
- **Label** (semibold): keybindings, status-bar segments, short affordances.
  Often paired with a muted key hint (e.g. `q` quit).

### Named Rules

**The Weight-Is-Hierarchy Rule.** When you need emphasis, change weight or
apply the muted modifier. Do not invent size scales the terminal cannot honor
or introduce a second typeface.

**The Monospace-Discipline Rule.** Alignment is part of the design: columns of
status glyphs, file modes, and line numbers align vertically because the font
is monospace. Preserve that alignment; never pad with proportional spaces or
mix in a proportional font.

## 4. Elevation

RightClick is **flat by default**. The terminal cannot render drop-shadows, and
this system does not simulate them. Depth is conveyed three ways, all tonal:

1. **Recession** — chrome (status bar, sidebar) sits on a background darker
   than the content canvas, so it falls back visually.
2. **Raise** — interactive surfaces (inputs) sit on a background lighter than
   the canvas, so they come forward.
3. **Border** — plain single-line box-drawing (`Borders::ALL`, default
   `BorderType::Plain`: `┌┐└┘─│`) defines boundaries. No rounded, double, or
   thick borders are used anywhere in the codebase.

Popups and overlays clear the cells behind them to the popup background and
enclose themselves with the chrome border; that is the only "lift" the system
expresses, and it is enough.

### Named Rules

**The Flat-By-Default Rule.** Never simulate shadows, glow, or blur. If a
surface needs to feel elevated, raise its background tone one step or border
it — nothing more.

**The Plain-Border Rule.** Borders are single-line and plain. Rounded, double,
and thick borders are not part of this system; introducing them breaks the
visual consistency across panes.

## 5. Components

### Borders & Chrome

- **Shape:** plain single-line box-drawing on all sides (`Borders::ALL`).
- **Color:** `{colors.border}` (`#414868`).
- **Use:** pane boundaries, popup frames, input outlines, notification frames.

### Lists (the primary surface)

- **Active item:** `{colors.highlight}` fill, `{colors.foreground}` text,
  1-cell horizontal padding. This is the focus indicator.
- **Inactive item:** recessed background, `{colors.muted}` text, same padding.
- **Selection marker:** an inline glyph (`▶` / `>` / `*`) in the accent color,
  so focus is visible even where highlight fill is not drawn.

### Popups / Overlays

- **Background:** `{colors.popup-bg}` (matches canvas); cells behind are
  cleared so the popup reads as a distinct layer.
- **Border:** `{colors.popup-border}`.
- **Padding:** 1 cell.
- **Centered** over the viewport; `Esc` closes.

### Inputs / Fields

- **Background:** `{colors.input-bg}` (raised fill, lighter than canvas).
- **Text:** `{colors.foreground}`; **placeholder:** `{colors.input-placeholder}`
  (muted — still contrast-checked, never the failing default gray).
- **Border:** plain chrome border; focus is conveyed by the cursor and by
  selection/highlight tone, not by a glow.

### Status Bar & Sidebar

- **Background:** `{colors.status-bar-bg}` (recessed, darker than canvas).
- **Text:** `{colors.status-bar-fg}`; key hints use the label weight with a
  muted modifier. The status bar is the lowest visual layer.

### Buttons (rare in a TUI)

- **Primary:** `{colors.button-bg}` fill with `{colors.button-fg}` (dark) text.
- **Hover:** swaps to the secondary accent
  (`{colors.button-hover-bg}` ≈ `#bb9af7`).
- **Note:** most "actions" in RightClick are keybindings, not buttons; reach
  for a button only when no keybinding exists.

### Git Status Indicators (signature)

File status is encoded by **color + glyph together**, never color alone:

- **Added** — `{colors.git-added}` (`#9ece6a`) with `+` / `A`.
- **Modified** — `{colors.git-modified}` (`#e0af68`) with `~` / `M`.
- **Removed** — `{colors.git-removed}` (`#f7768e`) with `-` / `D`.
- **Untracked** — `{colors.git-untracked}` (`#7aa2f7`) with `?`.

### Spinner / Progress

- A Unicode glyph sequence (e.g. `⠋⠙⠹`) cycled in the accent or info color to
  signal active work. Must respect reduced-motion (see §6).

## 6. Do's and Don'ts

### Do:

- **Do** design against the Default theme tokens first; treat the other three
  themes as palette swaps of the same roles.
- **Do** reinforce every semantic color with a glyph or position, so state is
  readable without color (The Never-Color-Only Rule).
- **Do** convey depth through tone: recessed chrome (`#16161e`), raised inputs
  (`#24283b`), plain borders (`#414868`).
- **Do** keep body and placeholder text contrast-checked against the canvas —
  muted is a tone, not a failure.
- **Do** use weight and the muted modifier for hierarchy; preserve monospace
  column alignment.
- **Do** provide a calm or static fallback for every spinner or transition
  under reduced-motion.

### Don't:

- **Don't** introduce gradients, glassmorphism, drop-shadows, glow, or any
  marketing polish transplanted from the web — RightClick is terminal-native.
- **Don't** encode git status or semantic state by color alone.
- **Don't** use rounded, double, or thick borders. Single-line plain borders
  only (`Borders::ALL`, `BorderType::Plain`).
- **Don't** let the primary accent cover more than a small fraction of a
  screen, and never let two equally-bright accents compete (The One Console
  Rule).
- **Don't** invent pixel font sizes, a second typeface, or proportional
  spacing — the terminal grid is fixed.
- **Don't** ship a spinner or animation without a reduced-motion fallback.
