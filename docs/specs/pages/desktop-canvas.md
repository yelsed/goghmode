# Desktop Canvas (macOS)

> Surface: `GoghModeApp` window · Source: `src/app.rs` · Status: done

## Goal & user
Someone at their Mac who wants a sketch in front of the agent right now, with no
second device involved. It is also the fallback surface when no iPad is nearby, and
— less obviously — it is the only thing that runs the local server and holds the
pairing token, so it must be open for the iPad and phone to work at all.

## Layout
Single window, vertical stack, dark chrome around a light canvas.

- Window 1100×760 on open, minimum 720×480.
- **Toolbar card** — rounded panel, fill `rgb(18,24,34)`, 1 px border
  `rgb(42,53,68)`, 16 px corner radius, 14/12 inner margin.
  - Row 1: title `GoghMode` (24 px, bold) + subtitle "Local sketchpad for Claude".
  - Row 2: brush slider, separator, Save / Undo / Clear, separator,
    Send to Claude / Copy image / Print prompt.
  - Row 3: Copy mobile URL, then either the live `Mobile` URL in monospace or the
    red "Mobile server unavailable".
- **Canvas** — fills remaining height minus 42 px, `egui::Frame::canvas`, paper fill
  `rgb(250,249,244)`, 1 px border `rgb(201,206,214)`, 18 px corner radius.
- **Status bar** — one line of 13 px text in `rgb(173,184,199)`.

## Components
Immediate-mode `egui` widgets — there is no component library and no widget tree.

- `egui::Slider` for the brush, 1–32, step 1.
- `primary_button` helper for the two accent actions (Save, Send to Claude); plain
  `ui.button` for the rest.
- `egui::Painter` for the canvas — every stroke is repainted each frame.
- `StatusBar` — a one-method struct, not a widget.
- `configure_visuals` hand-sets the dark palette; there is no theme system.

### Sub-component specs
- [export-contract](../components/export-contract.md) — what Save actually writes.
- [mobile-server-api](../components/mobile-server-api.md) — the server this window owns.

## Design tokens
Hardcoded in `src/app.rs`; there is no token file.

| Token | Value |
| --- | --- |
| Window / panel fill | `rgb(10,14,20)` |
| Toolbar fill / border | `rgb(18,24,34)` / `rgb(42,53,68)` |
| Paper | `rgb(250,249,244)` |
| Canvas border | `rgb(201,206,214)` |
| Ink | `rgb(17,24,39)` (`#111827`) |
| Title / body / muted text | `rgb(245,247,250)` / `rgb(214,220,230)` / `rgb(165,176,192)` |
| Corner radius | 16 toolbar · 18 canvas |
| Item spacing | 10×10 |

## Tech used
- **UI:** `eframe` 0.34 with `App::ui(&mut self, ui, frame)` — the newer trait
  method, not `update(ctx, frame)`.
- **Event loop:** `run_and_return: false`. Returning from the event loop is
  unreliable on macOS; a unit test asserts the flag stays false.
- **Repaint:** explicit `request_repaint()` while dragging only. The window is
  otherwise idle.
- **Clipboard:** `arboard`, for both prompt text and raw RGBA image bytes.

## Auth & access
None. Local application, no accounts, no permissions. The window does hold the
pairing token indirectly — starting the app starts the server, closing it stops the
server through `Drop`.

## Data
- **Owns:** `Drawing` — committed strokes plus one optional in-progress stroke.
- **Writes:** `export::write_snapshot(&snapshot, &drawings_dir)` → `latest.json`,
  `latest.svg`, `latest.png`.
- **Reads:** nothing. The canvas never loads a previous drawing back; opening the
  app gives an empty page.
- **Types:** `DrawingSnapshot`, `Stroke`, `Point`, `CanvasSize` from `src/drawing.rs`.

## Client state
`GoghModeApp { drawing, drawings_dir, mobile_server: Option<MobileServer>, status: String }`.
All of it is client state — there is no server state to mirror.

## Input handling
The canvas rectangle is allocated with `Sense::drag()`, and its size is pushed back
into the model every frame (`set_canvas_size`). This is why exported canvas
dimensions are the live widget size (e.g. 1100 × 699.5), not the model's initial
1024 × 640.

| Event | Behaviour |
| --- | --- |
| `drag_started` | `begin_stroke(x, y, 0.5, unix_millis())` |
| `dragged` | `push_point(...)` + `request_repaint()` |
| `drag_stopped` | final `push_point`, `finish_stroke`, then **autosave** |

Pointer positions are converted to canvas-local coordinates and dropped if outside
the rectangle. Pressure is hardcoded `0.5` — a mouse or trackpad has none.
Consecutive identical points are deduplicated; empty strokes are discarded.

## Toolbar actions

| Action | Behaviour |
| --- | --- |
| Brush | 1–32, applied to the next stroke. Existing strokes keep their width. |
| Save | Writes the snapshot immediately. Redundant during normal drawing (autosave already ran) but useful after Undo/Clear. |
| Undo | Drops the last committed stroke, then saves. |
| Clear | Drops everything, then saves — so the exported files become an empty page too. |
| Send to Claude | Copies the Claude-flavoured prompt to the clipboard. |
| Copy image | Rasterizes the current snapshot and puts RGBA pixels on the clipboard. |
| Print prompt | Prints the generic prompt to stdout. |
| Copy mobile URL | Copies `http://{lan_ip}:{port}/{token}/`. |

## States

| State | Behaviour |
| --- | --- |
| Default | Empty paper canvas, status line shows the drawings directory and mobile URL. |
| Drawing | Active stroke painted live alongside committed strokes. |
| Saved | Status line reports the write; no modal, no toast. |
| Mobile server unavailable | Toolbar shows red "Mobile server unavailable"; drawing and local export still work. Non-fatal by design. |
| Save error | Status line carries the error text. The previous `latest.*` files are untouched — the write is atomic. |

## Estimate
Shipped. Only remaining work is listed.

| Scope | Estimate |
| --- | --- |
| Layout, toolbar, canvas, status bar | shipped |
| Input handling & autosave | shipped |
| Clipboard actions | shipped |
| Page switcher / browser (Phase 1 & 3) | not estimated — see [PLANNING.md](../../PLANNING.md) |
| QR pairing panel (Phase 2) | not estimated |
| **Total** | — |

## Tasks
- [ ] Warn when the server did not get port 8787 instead of silently using another.
- [ ] Decide what this window becomes once the iPad is the primary surface — see
      Phase 3 in [PLANNING.md](../../PLANNING.md).

## Open questions
- Should Undo be repeatable beyond one step, and should there be a redo?
- Should the desktop canvas gain colour selection, given the PNG exporter currently
  ignores `stroke.color`?
