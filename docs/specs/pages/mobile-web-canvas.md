# Mobile Web Canvas

> Route: `http://{lan_ip}:{port}/{token}/` · Source: `mobile/index.html` · Status: done

## Goal & user
Anyone with a phone or tablet on the same Wi-Fi who wants to draw and get it to the
Mac without installing anything. It is the zero-friction path — no app store, no
signing, no account — and the fallback for devices the native companion does not
cover.

## Layout
One self-contained HTML file: inline CSS, inline JavaScript, no build step, no
framework.

- **Header** — eyebrow "Local sketchpad", `h1` GoghMode, and a status paragraph with
  `role="status"` / `aria-live="polite"`.
- **Toolbar** — primary actions (Send to desktop, Share PNG) and secondary actions
  (Undo, Clear, Export SVG, Export JSON, Export PNG, brush slider 1–32).
- **Canvas** — `aspect-ratio: 8 / 5`, `min-height: 330px`,
  `max-height: calc(100svh - 230px)`, 24 px radius, cream paper, soft shadow.
- Header collapses to a column below 560 px.

## Components
Plain HTML elements. No component library.

- `<canvas id="canvas" width="1024" height="640">` — fixed backing store, scaled by CSS.
- `<input type="range">` for the brush with an `<output>` readout and an
  `aria-label`.
- `<button>` for every action; the status paragraph is the only feedback channel.

### Sub-component specs
- [mobile-server-api](../components/mobile-server-api.md) — where **Send to desktop** posts.
- [export-contract](../components/export-contract.md) — what the Mac writes on receipt.

## Design tokens
Inline CSS custom properties, `oklch()` throughout.

| Token | Value |
| --- | --- |
| Paper | `rgb(250, 249, 244)` |
| Ink | `rgb(30, 35, 48)` |
| Canvas border | `oklch(38% 0.025 260)` |
| Canvas shadow | `0 22px 72px oklch(8% 0.035 260 / 0.42)` |
| Canvas radius | 24 px |
| Brush default / range | 4 · 1–32 |

Note the ink here is `rgb(30,35,48)`, slightly different from the desktop's
`#111827`. Cosmetic only — the Mac re-renders the PNG in its own ink anyway.

## Tech used
- **Input:** Pointer Events only (`pointerdown/move/up/cancel`) with
  `setPointerCapture` and a single `activePointerId` guard, so a second finger
  cannot corrupt a stroke. `touch-action: none` plus user-select and tap-highlight
  suppression stop the browser scrolling or selecting mid-stroke.
- **Rendering:** full `repaint()` on every move — clear, replay all strokes. Fine at
  sketch scale, and simpler than dirty-rectangle bookkeeping.
- **Upload:** `fetch("save", { method: "POST" })` — a **relative** URL, which is why
  the token never appears in the JavaScript. Whatever secret path served the page
  is the path it posts to.
- **Offline shell:** `service-worker.js`, cache-first, cache name
  `goghmode-mobile-v2`. It caches the shell only and must never cache
  `drawings/latest*` or `goghmode-latest*` — a test asserts this.
- **Installable:** `manifest.webmanifest`, `display: standalone`, with **relative**
  `start_url` and `scope` so it works under any token path.

## Auth & access
The secret path is the only credential, and the page never handles it — see the
trust model in [ARCHITECTURE.md](../../ARCHITECTURE.md) and
[ADR-0002](../../decisions/0002-token-in-path-lan-pairing.md).

## Data
- **Sends:** the `DrawingSnapshot` shape, plus an extra `files` block the iPad does
  not send. Harmless — serde ignores unknown fields and the Mac writes its own
  `files` block on export.
- **Canvas metadata:** `background` is `"rgb(250, 249, 244)"` here; the Mac's SVG
  exporter hardcodes `#ffffff`, so the paper tone is dropped.
- **Point fields:** `pressure` from the event, falling back to `0.5` when the device
  reports 0; `t` as epoch milliseconds
  (`performance.timeOrigin + performance.now()`).
- **Local exports** build their own SVG in the browser — a deliberate near-duplicate
  of the Rust exporter — and download as `goghmode-latest.{svg,json,png}`. These
  never touch the Mac's drawings directory.

## Client state
Module-scope variables: `strokes[]`, `activeStroke`, `activePointerId`, `nextId`.
No framework, no store, nothing persisted between reloads.

## Behaviour details
- Coordinates scale CSS pixels to canvas pixels through `getBoundingClientRect`.
- Out-of-bounds and duplicate points are dropped on append.
- Pressure is captured but **not used for rendering** — `lineWidth` is the flat
  brush value.
- Undo is `strokes.pop()`. There is no redo.
- Ink is fixed; there is no colour picker.
- **Share PNG** uses `navigator.share` where available, with a download fallback.

## States

| State | Behaviour |
| --- | --- |
| Default | "Draw on the paper. Send to desktop when ready." |
| Drawing | Stroke drawn live; no status change. |
| Sending | Send button disabled for the duration of the request. |
| Sent | Status: success, and a nudge to type `/goghmode` in Claude Code. |
| Send failed | Status explains what to check — keep GoghMode open on the Mac, same Wi-Fi. |
| Rejected (400) | The server's named reason is the response body; the status line surfaces it rather than a generic failure. |
| Offline / installed as a progressive web app | Shell loads from cache; **Send to desktop** fails until the host is reachable. |

## Estimate
Shipped. Only remaining work is listed.

| Scope | Estimate |
| --- | --- |
| Markup, styling, canvas | shipped |
| Pointer handling & rendering | shipped |
| Send to desktop + local exports | shipped |
| Progressive web app shell | shipped |
| Page switcher (Phase 1) | not estimated — see [PLANNING.md](../../PLANNING.md) |
| **Total** | — |

## Tasks
- [ ] Bump the service worker cache name whenever `mobile/index.html` changes, or
      switch the shell to stale-while-revalidate. Today an installed app keeps the
      old HTML indefinitely.
- [ ] Consider coalesced and predicted pointer events where Safari exposes them, for
      rapid pen lift and restart.

## Open questions
- Should the web canvas gain a colour picker, given the iPad has one and the PNG
  exporter ignores colour anyway?
- Should it warn when running over insecure HTTP where high-quality pointer APIs are
  unavailable?
