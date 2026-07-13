# Appearance Theming Design (UI themes + SSH terminal fonts/colors)

## Goal

Replace the current three-option theme (`system` / `dark` / `light`) with a
richer preset-based appearance system. One selection drives both the app
chrome (sidebar, panels, buttons) and the SSH terminal's color scheme.
Terminal font family and size become independently configurable, on top of
whichever theme is selected.

## Scope decision

App theme and terminal color scheme are **one unified setting**, not two
independent ones. Picking "Dracula" recolors the whole app and the terminal
at once. This matches how terminal-forward apps (Warp, Hyper) work and keeps
the settings surface small.

## Theme presets

`system`, `dark`, `light` (existing, unchanged) plus six new presets:
`dracula`, `nord`, `solarized_dark`, `solarized_light`, `monokai`, `one_dark`.

`system` keeps its current meaning: it resolves to `dark` or `light` based on
OS preference (`prefers-color-scheme`), exactly as today. It has no analog
for the six new presets — they are always explicit choices.

## Architecture

Two single-purpose, additive registries, kept in sync by a test:

1. **App chrome (CSS):** each preset gets its own
   `:root[data-theme="<id>"] { --bg-0: ...; --text-0: ...; --accent: ...; }`
   block in `src/styles.css`, following the exact pattern of the existing
   `dark`/`light` blocks. No new CSS variables, no refactor of existing
   working CSS — purely additive blocks.
2. **Terminal palette (TypeScript):** a new `src/terminalThemes.ts` registry
   maps each concrete theme id (`dark`, `light`, `dracula`, ...; never
   `system`) to an xterm.js `ITheme` object (background, foreground, cursor,
   selectionBackground, and the 16 ANSI colors).

Rejected alternative: deriving the terminal palette from CSS variables at
runtime. App chrome only needs ~10 colors; a terminal needs a full 16-color
ANSI set. Bolting that onto the CSS variable surface would pollute it for
every other component that reads those variables. Two small registries, each
holding only the colors it needs, is cleaner.

A theme switch does two things: set `data-theme` on `<html>` (recolors chrome
via CSS, exactly as `bootstrapSystemTheme`/`resolveTheme` do today, just
generalized past two hardcoded options) and look up the resolved id in
`terminalThemes.ts` to recolor any open terminal tabs.

**Live update:** `TerminalTab.tsx` currently hardcodes `fontFamily`,
`fontSize`, and `theme` at construction. It will instead read them from the
settings store and assign them to the live `Terminal.options` object
(supported since xterm.js v5) whenever settings change, so open terminal tabs
update immediately without a reconnect.

## Settings contract changes

`schema_version` stays `1` — this app has no migration machinery yet and no
real users, so new fields get `#[serde(default = ...)]` (Rust) /
optional-with-default handling (TS) instead of a version bump.

New / changed fields in `AppSettings` (both `src-tauri/src/settings.rs` and
`src/settings.ts`):

| Field | Type | Default | Valid values |
| --- | --- | --- | --- |
| `theme` | enum/string | `system` | `system`, `dark`, `light`, `dracula`, `nord`, `solarized_dark`, `solarized_light`, `monokai`, `one_dark` |
| `terminal_font_family` | string | `"JetBrains Mono", "DejaVu Sans Mono", monospace` | non-empty after trim, max 200 chars |
| `terminal_font_size_px` | integer | `13` | `9..=24` |

Validation mirrors on both Rust and TS sides, same pattern as the existing
settings fields (exact-match enum membership, inclusive numeric bounds).

## Settings UI

New "Appearance" section in `SettingsModal.tsx`:

- **Theme:** a grid of swatch buttons, one per preset, each rendered with a
  few of its actual colors as a small preview (not a plain radio list).
- **Terminal font:** a dropdown with curated common monospace fonts
  (JetBrains Mono, Fira Code, Cascadia Code, Hack, Source Code Pro, Ubuntu
  Mono, DejaVu Sans Mono) plus a **Custom…** option that reveals a free-text
  input for any installed font family string. Whatever is chosen, the value
  applied to xterm always has `, monospace` appended as a final fallback (if
  not already present), so an uninstalled/misspelled font degrades to *some*
  monospace font instead of a stray proportional one.
- **Terminal font size:** a number input, `9`–`24`.

Persistence reuses the existing settings flow exactly: optimistic update in
the Zustand store, roll back to the previous persisted value if backend
validation or save fails.

## Testing

- Rust (`settings.rs`): extend existing validation tests for the new theme
  enum variants and the font family/size bounds (valid + invalid cases,
  inclusive boundaries).
- Frontend: a test asserting every theme id (except `system`) has an entry in
  `terminalThemes.ts`, so a new CSS-only preset can't ship without its
  terminal palette (and vice versa). Extend `settings.test.ts`-style
  validation tests for the two new fields.
- Manual smoke: cycle through each preset in Settings and confirm app chrome
  and an already-open terminal tab recolor together; change font family and
  size and confirm an open terminal tab updates live, no reconnect.

## Non-goals (v1)

- Per-server theme/font overrides (one global appearance setting).
- A custom palette editor / user-defined themes.
- Theme import/export.
- Font ligatures or Nerd Font glyph-specific handling.
