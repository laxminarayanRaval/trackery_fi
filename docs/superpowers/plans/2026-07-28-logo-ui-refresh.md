# Logo & UI/UX Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the placeholder app icon with a custom ₹-monogram mark and restyle the transaction UI with a charcoal/terracotta "classical modern" look, using only system fonts and existing tooling.

**Architecture:** Pure front-end/asset change. One new vector master icon (`apps/mobile/app-icon.svg`) regenerated into all platform icon sizes via the Tauri CLI's own `tauri icon` generator. CSS design tokens in `:root` drive the palette so every rule reads from one source. `App.tsx` gains a header element reusing the same monogram as an inline SVG (no extra image asset for in-app use).

**Tech Stack:** Existing stack only — React 19 + TypeScript + Vite, plain CSS, `@tauri-apps/cli` (`tauri icon`). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-07-28-logo-ui-refresh-design.md`

---

### Task 1: Master icon SVG + regenerate platform icons

**Files:**
- Create: `apps/mobile/app-icon.svg`
- Modify (generated, not hand-edited): `apps/mobile/src-tauri/icons/*`, `apps/mobile/app-icon.png`

- [ ] **Step 1: Write the master SVG**

`apps/mobile/app-icon.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <rect width="1024" height="1024" fill="#241F1B"/>
  <path d="M 372 300 H 652
           M 372 300 V 420
           M 372 420 H 612
           M 452 420 L 700 720"
        fill="none" stroke="#B9542E" stroke-width="72"
        stroke-linecap="round" stroke-linejoin="round"/>
</svg>
```

This draws a simplified, custom-geometric ₹ monogram (top bar, vertical
stem, second bar, diagonal leg) in terracotta on a charcoal square. Not a
rendered font glyph, so it stays crisp at favicon size and matches no
default system "₹".

- [ ] **Step 2: Regenerate platform icons from the SVG**

Run from `apps/mobile`:

```bash
npx tauri icon app-icon.svg -o src-tauri/icons
```

Expected: command exits 0 and rewrites files under `src-tauri/icons/`
(icon.png, icon.ico, icon.icns, the `ios/AppIcon-*.png` set, and Android
sizes if present in that directory already).

- [ ] **Step 3: Refresh the checked-in PNG master to match**

The convention in this repo is `app-icon.png` as the human-viewable master
next to the SVG. Regenerate it from the same SVG at 1024×1024:

```bash
npx tauri icon app-icon.svg -o /tmp/icon-check
cp /tmp/icon-check/icon.png app-icon.png
rm -rf /tmp/icon-check
```

If `icon.png` in the output isn't 1024×1024, use whichever generated file
in that directory is largest/closest to 1024×1024 square.

- [ ] **Step 4: Visually verify**

Read `apps/mobile/app-icon.png` back (image view) and confirm: charcoal
square, centered terracotta ₹ mark, no artifacts, legible shape at a glance.

- [ ] **Step 5: Commit**

```bash
git add apps/mobile/app-icon.svg apps/mobile/app-icon.png apps/mobile/src-tauri/icons
git commit -m "feat(mobile): custom rupee-monogram app icon

Replace the placeholder ring-on-blue icon with a hand-drawn rupee
monogram (charcoal ground, terracotta mark) generated via the Tauri
CLI's own icon pipeline — no new tooling."
```

---

### Task 2: CSS design tokens + full stylesheet rewrite

**Files:**
- Modify: `apps/mobile/src/App.css`

- [ ] **Step 1: Replace the file contents**

Full replacement for `apps/mobile/src/App.css`:

```css
:root {
  --color-paper: #f6f1e9;
  --color-paper-raised: #fffdfa;
  --color-ink: #241f1b;
  --color-ink-muted: #79695a;
  --color-border: #e2d8c8;
  --color-accent: #b9542e;
  --color-accent-ink: #fbf6ee;
  --color-error: #8a3324;
  --color-notice: #4c6b44;

  --font-serif: ui-serif, Georgia, Cambria, "Times New Roman", serif;
  --font-sans: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;

  --radius: 6px;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  font-family: var(--font-sans);
  font-size: 14px;
  background: var(--color-paper);
  color: var(--color-ink);
}

.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  gap: 12px;
}

.brand-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-paper-raised);
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
}

.brand-mark {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  flex-shrink: 0;
}

.brand-name {
  font-family: var(--font-serif);
  font-size: 19px;
  letter-spacing: 0.01em;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
}

button {
  font: inherit;
  padding: 8px 16px;
  border: 1px solid var(--color-accent);
  border-radius: var(--radius);
  background: var(--color-accent);
  color: var(--color-accent-ink);
  cursor: pointer;
}

button:disabled {
  opacity: 0.5;
  cursor: default;
}

button:not(:disabled):hover {
  filter: brightness(1.08);
}

.status {
  color: var(--color-ink-muted);
  font-size: 13px;
}

.content {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 0 16px 16px;
}

.password-box {
  border: 1px solid var(--color-border);
  border-radius: var(--radius);
  background: var(--color-paper-raised);
  padding: 14px 16px;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.password-box p {
  margin: 0;
  flex-basis: 100%;
  color: var(--color-ink-muted);
}

.password-box input {
  font: inherit;
  padding: 8px 10px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius);
  background: var(--color-paper);
  color: var(--color-ink);
}

.error {
  margin: 0;
  color: var(--color-error);
}

.notice {
  margin: 0;
  color: var(--color-notice);
}

/* One scroll container for both axes; the virtualizer reads its scrollTop. */
.table-scroll {
  flex: 1;
  overflow: auto;
  border: 1px solid var(--color-border);
  border-radius: var(--radius);
  background: var(--color-paper-raised);
}

.table {
  min-width: 980px;
}

.row {
  display: grid;
  grid-template-columns: 95px 260px 170px 140px 115px 115px 60px 65px;
  border-bottom: 1px solid var(--color-border);
}

.row:not(.head):hover {
  background: color-mix(in srgb, var(--color-accent) 6%, transparent);
}

.row.head {
  position: sticky;
  top: 0;
  background: var(--color-paper-raised);
  color: var(--color-ink-muted);
  font-weight: 600;
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  border-bottom: 1px solid var(--color-ink);
  z-index: 1;
}

.cell {
  padding: 9px 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cell.num {
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.empty {
  padding: 20px 16px;
  color: var(--color-ink-muted);
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/mobile/src/App.css
git commit -m "style(mobile): charcoal/terracotta design tokens and refreshed table"
```

---

### Task 3: Wire the header/logo mark and favicon into the app shell

**Files:**
- Modify: `apps/mobile/src/App.tsx`
- Modify: `apps/mobile/index.html`

- [ ] **Step 1: Add a header with the inline monogram, above the existing toolbar**

In `apps/mobile/src/App.tsx`, replace the return block's opening (`<main
className="app">` through the closing of `.toolbar`, i.e. lines 139–146 of
the current file) with:

```tsx
  return (
    <main className="app">
      <div className="brand-bar">
        <div className="brand">
          <svg className="brand-mark" viewBox="0 0 1024 1024" aria-hidden="true">
            <rect width="1024" height="1024" fill="#241F1B" />
            <path
              d="M 372 300 H 652 M 372 300 V 420 M 372 420 H 612 M 452 420 L 700 720"
              fill="none"
              stroke="#B9542E"
              strokeWidth="72"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          <span className="brand-name">trackery_fi</span>
        </div>
        <div className="toolbar">
          <button onClick={() => void pick()} disabled={busy || pendingPath !== null}>
            Pick statement PDF
          </button>
          {busy && <span className="status">Working…</span>}
        </div>
      </div>

      <div className="content">
```

The path data is the same monogram as `app-icon.svg` (Task 1) so the
in-app mark and the OS-level app icon are visually identical.

- [ ] **Step 2: Close the new wrapper and keep the rest of the JSX unchanged**

The existing JSX after the toolbar (password form, error/notice, table)
now needs to sit inside the new `.content` div opened in Step 1. Change
the final closing tags of the component (currently the last `</div>`
before `</main>`) from:

```tsx
      </div>
    </main>
  );
}
```

to:

```tsx
      </div>
      </div>
    </main>
  );
}
```

(one extra closing `</div>` to close `.content`).

- [ ] **Step 3: Add a favicon**

In `apps/mobile/index.html`, add a favicon link referencing the SVG master
so the browser/dev-server tab shows the same mark:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <link rel="icon" type="image/svg+xml" href="/app-icon.svg" />
    <title>trackery_fi</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

Vite serves files from the project root by default, and `app-icon.svg`
already lives at `apps/mobile/app-icon.svg` (the project root for this
package), so `/app-icon.svg` resolves without copying the file anywhere.

- [ ] **Step 4: Commit**

```bash
git add apps/mobile/src/App.tsx apps/mobile/index.html
git commit -m "feat(mobile): header with monogram mark, wire favicon"
```

---

### Task 4: Verify

**Files:** none (verification only)

- [ ] **Step 1: Type-check and build**

```bash
cd apps/mobile
npm run build
```

Expected: exits 0 (runs `tsc` then `vite build`), no TypeScript errors
about the new JSX.

- [ ] **Step 2: Visual check**

```bash
npx tauri dev
```

Confirm: header shows the charcoal/terracotta mark and "trackery_fi"
wordmark in serif, the import button is terracotta, the table header row
has muted uppercase labels with a dark bottom border, rows show a faint
accent-tinted hover, and the password/error/notice states use the new
palette. Close the dev window when done.

- [ ] **Step 3: No commit** (verification only; if either check fails, fix
      the relevant file from Task 2/3 and re-run before moving on)

---

### Task 5: Push

- [ ] **Step 1: Push the branch**

```bash
git push
```

Expected: pushes the new commits to `worktree-sprint1-continue` (already
tracks a remote per `NEXT_SESSION.md`). No PR — this repo's convention
(per `NEXT_SESSION.md`) is commits-only, no PR opened, matching Sprint 1.
