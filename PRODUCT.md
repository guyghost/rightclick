# Product

## Register

product

## Users

Developers who work with AI coding agents (Claude, Cursor, Codex, and others) and
currently juggle a dozen terminal tabs, editor windows, and agent UIs. They live in the
terminal for long, focused sessions and treat context-switching as a tax on attention.

The job to be done: keep the entire development workflow — planning tasks, talking to
agents, reviewing diffs, staging commits, browsing past conversations, navigating files,
and switching worktrees — in one calm, keyboard-driven surface, so they never leave the
shell to do work the shell does better.

## Product Purpose

RightClick is a terminal UI dashboard that unifies the AI-agent development workflow in a
single shell. It exists because wrangling multiple agents across scattered tools fragments
attention and slows experts down.

Success looks like this: a developer opens RightClick at the start of a session and does
not close it. They plan, delegate to agents, inspect output, diff, commit, and hop between
worktrees with a handful of keystrokes — faster than reaching for the mouse, and with the
whole context visible at once.

## Brand Personality

Sharp. Precise. Power-user.

Voice and tone are direct, terse, and expert — like a well-tuned instrument an operator
trusts. No hand-holding, no cheerleading, no decoration that doesn't earn its place. The
emotional goal is the feeling of a focused cockpit: everything present, nothing competing,
the operator in control.

## Anti-references

- **Noisy, glossy SaaS dashboards.** No gradients, glassmorphism, soft drop-shadows, or
  marketing polish transplanted from the web. RightClick is terminal-native; effects that
  read as "consumer web app" are a failure signal, not a feature.
- **Decorative-for-its-own-sake chrome.** Anything that exists to look impressive rather
  than to convey state or afford action does not belong.

## Design Principles

1. **Terminal-native, not transplanted web.** Every visual choice respects the medium:
   character cells, contrast, and density — not CSS effects, shadows, or blur. The
   constraint is the aesthetic.
2. **Density without noise.** Show a lot, but never compete for attention. One clear focus
   at a time; supporting information recedes until it is needed.
3. **The keyboard is the interface.** Every action is reachable, fast, and discoverable.
   The mouse is never required, and the design never assumes it.
4. **Show, don't decorate.** Information and state carry the design. An expert operator
   should be able to read the screen at a glance; ornament gets in the way.
5. **Practice what you preach.** RightClick orchestrates AI agents from a model-driven,
   testable core (Functional Core & Imperative Shell). Its own interface must model the
   same discipline — explicit state, predictable transitions, nothing accidental.

## Accessibility & Inclusion

- **Git status is never color-only.** Added / removed / modified / untracked states must be
  distinguishable by glyph, position, or label as well as hue, so color vision never gates
  understanding.
- **Reduced motion is respected.** Any spinner, transition, or animation has a calm or
  static fallback; motion conveys state, never decoration.
- **Strong terminal contrast everywhere.** Body text, muted text, and placeholder text meet
  contrast expectations across every shipped theme; muted grays that fail against the
  background are treated as bugs.
- **Color-blind-safe palettes.** Themes must remain legible and unambiguous under common
  color vision deficiencies (protanopia, deuteranopia, tritanopia).
