# Logo & UI/UX refresh — design

## Purpose

The Sprint 1 app icon is a placeholder (a plain white ring on flat blue — no
financial identity) and the transaction UI is deliberately unstyled system
defaults. User asked for a better logo and a more "classical modern" look,
checked properly before shipping.

This is a visual-only pass: no new screens, no new dependencies, no change to
data flow or the Rust core. Scope is `apps/mobile` (App.tsx/App.css,
index.html) and the app icon source + generated platform icons.

## Direction (confirmed with user)

- **Mark:** custom ₹ (rupee) monogram — reads as "money app" without being a
  generic glyph or clip-art rupee sign.
- **Palette:** charcoal ink + a single terracotta accent on a warm paper
  background. No multi-color scheme, no gradients. Muted (not saturated)
  status colors for error/notice so the one accent color stays distinct.
- **Type:** system font stacks only (offline-first app, no webfont fetch,
  no new deps) — a serif stack for the wordmark/headings, the existing
  sans stack for body/table text, tabular numerals for amounts (already
  used).
- **Scope:** full visual pass on everything currently on screen — header/
  branding, toolbar button, password card, table (header row, rows, empty/
  error/notice states) — layout structure (virtualized table, form flow)
  is unchanged.

## Logo

- Author as SVG at a fixed 1024×1024 viewBox: charcoal square/rounded-square
  ground, terracotta ₹ monogram centered, custom-drawn (not system font "₹")
  so it holds up at 16px favicon size.
- Rasterize to `apps/mobile/app-icon.png` (1024×1024).
- Regenerate all platform icons (`src-tauri/icons/*`, iOS set) from that PNG
  via the Tauri CLI's own icon generator (`npx tauri icon`) — this is the
  same tool that produced the current icon set, so no new tooling.
- Add a small inline SVG (or the same PNG) as favicon + in-app header mark,
  reusing the one master asset rather than hand-crafting a second logo.

## UI

- **Palette (CSS custom properties in `:root`):** paper background, charcoal
  ink, terracotta accent, muted borders, muted error/notice colors. Single
  source so the values aren't repeated across rules.
- **Header:** new bar with the logo mark + "trackery_fi" wordmark in the
  serif stack, replacing the bare toolbar as the top of the page. Import
  button and status text move here.
- **Buttons:** terracotta fill, paper text, subtle radius, no heavy shadows
  (flat/editorial, not skeuomorphic). Disabled state dims via opacity.
- **Password card:** bordered card with radius/padding consistent with the
  rest of the shell, instead of a bare bordered box.
- **Table:** refined header row (letter-spacing, muted bottom border),
  lighter row borders, subtle row hover, tabular nums kept on amount/balance
  columns.
- **Error/notice:** muted brick-red / muted moss-green text, not saturated
  red/green, so the terracotta accent remains the only "loud" color.

## Out of scope

- No dark mode / theming system (doctrine: no theming system for Sprint 1).
- No new fonts, icon libraries, or CSS frameworks.
- No changes to `crates/trackery-core` or Tauri commands.
- No change to the table's columns or virtualization behavior.

## Verification

- `npm run build` (tsc + vite build) in `apps/mobile` succeeds.
- Visual check: launch via `npx tauri dev` (or the Windows exe path already
  proven in this worktree) and confirm the header, button, password flow,
  and table render as designed at a typical desktop window size.
- `npx tauri icon` output committed (icons regenerated from new master PNG).
