# The desktop app becomes a bridge

> **Status:** design, approved 26 July 2026 · **Audience:** developers
> Resolves [PLANNING.md](../../PLANNING.md) Phase 3, "What the Mac app becomes".

## Why

Two complaints, one week apart, that turn out to be the same complaint.

**"The Mac app feels a bit redundant."** Once the iPad is the good drawing surface, the desktop
canvas is the worse way to draw. Recorded in `later.md`, deferred as Phase 3.

**"The UI is a bit clunky and full."** One toolbar serves three views, so it carries the union of
everything they need: a brush slider, Save, Undo, Clear, Send to agent, Copy image, Print prompt,
Copy mobile URL, the full mobile URL in monospace, and a port warning — all permanently on screen,
including on the register and the devices panel where most of it means nothing.

Underneath both: the desktop is trying to be a drawing app *and* the bridge, and the chrome is
sized for the first job while the value is in the second.

A third thing surfaced while looking: **opening the app twice starts two servers.** The bundle
launcher `nohup`s a fresh process every time, so `open` never focuses an existing window. Only one
process can hold 8787; the rest silently fall back to an ephemeral port. Observed live with three
instances on 8787, 55749 and 51796, all writing to the same drawings directory. Every saved
connection points at 8787, so the losers are unreachable while looking perfectly healthy.

## Decisions

1. **The desktop stops being a drawing surface.** The canvas, brush, undo, clear and save are
   removed outright — not demoted.
2. **The window is the register, the connection, and pairing.** Nothing else.
3. **8787 is reserved and reused.** A second launch detects the first and hands over to it rather
   than starting a rival server. The ephemeral-port fallback is removed.
4. **One visual language.** The desktop adopts the `DESIGN.md` tokens the iPad already uses,
   following the system light/dark appearance.

## The window

```
┌──────────────────────────────────────────────┐
│  GoghMode            ● Connected · MacBook   │
│                                              │
│  SHEET  NAME              UPDATED   AGENT    │
│  ──────────────────────────────────────────  │
│  01     26 Jul 19:19      2 min     ISSUED   │
│  02     Server sketch     1 hr               │
│  03     mac-scratch       3 hr               │
│                                              │
│  [ Devices ]                     [ Folder ]  │
└──────────────────────────────────────────────┘
```

**Register (home).** The existing paper table from `draw_page_browser`, unchanged in substance — it
is the good part and was built recently. It becomes the only view you land on.

**Connection chip.** Replaces an entire row of furniture: `Copy mobile URL`, the word "Mobile", the
URL in monospace, and the port warning. Collapses to `● Connected · <host name>` in the header.

Concretely an `egui::menu::menu_button` (or `popup_below_widget`) whose panel holds the mobile URL,
a copy button, and any port problem — chosen because egui has no popover primitive and a collapsing
section would push the register down every time it opened. The URL belongs beside pairing anyway:
both exist to get a device talking to this host.

**Devices.** Unchanged from what shipped: pairing, the device list, revocation, the legacy toggle.

### Removed

| Control | Why it goes |
| --- | --- |
| Canvas, brush, Save, Undo, Clear | The desktop stops drawing. |
| Send to agent, Print prompt | Both copy or print a text prompt telling an agent where to look. `/goghmode` reads the files directly, and `goghmode prompt` still exists on the command line. |
| Copy image | There is no current drawing to copy once the canvas is gone. Not replaced by a per-sheet action. |
| `port_warning()` | Nothing falls back to another port any more; the failure it described cannot happen. |

## Port ownership

`MobileServer::start` gains one decision, in this order:

| Situation | Behaviour |
| --- | --- |
| 8787 is free | Bind it. Run normally. |
| 8787 answers as GoghMode | An instance is already running. Best-effort bring its window forward, then exit `0`. No second window, no second server, the saved connection is untouched. |
| 8787 is held by something else | Run without a server and say so plainly, naming the port. |

**Detection.** Binding *is* the lock — no lock file, no PID staleness, nothing to leave behind after
a crash. When the bind fails, `GET http://127.0.0.1:8787/v2/hello` with a short timeout decides who
holds it: a body advertising the `pairing-v2` feature is one of ours.

*Known limitation:* an instance running the pre-pairing build has no `/v2/hello` and answers 404, so
it reads as foreign. It resolves itself the first time both processes are the current build, and the
message names the port either way.

**Focusing the existing window** is `osascript -e 'tell application "GoghMode" to activate'`, run
best-effort and ignored on failure. The bundle launcher starts the binary detached with `env -i`, so
it is not reliably addressable as an application — exiting cleanly is the guarantee, raising the
window is the courtesy.

## Theme

`configure_visuals` drops the navy (`#0A0E14`) and adopts `DESIGN.md`: ground `#EDEAE4`, sheets
`#FFFFFF`, ink `#1A1917`, rules `#A8A29A`, stamp `#B4331F`. `DESIGN.md` already defines the dark
pairs — `register-ground-dark`, `sheet-dark`, `ink-dark` — so the window follows the system
appearance instead of being permanently dark.

Verified available in egui 0.34: `ctx.set_theme(ThemePreference::System)` selects the appearance,
and `ctx.set_visuals_of(Theme::Light, …)` / `set_visuals_of(Theme::Dark, …)` supply a palette for
each. Both need registering **once** rather than per frame — today `configure_visuals` runs from
`ui()` on every frame and hard-sets `Visuals::dark()`, which is what makes the theme unconditional.

The desktop constants added by the drawing-set work (`GROUND`, `PAPER`, `SHEET_EDGE`, `RULE`, `INK`,
`INK_LABEL`, `STAMP`) are the light half already. The dark half is new and comes from `DESIGN.md`;
neither surface should carry a colour the other does not know about.

Today the register is paper inside a navy shell, which is most of why it reads as clunky: the app
looks like two apps.

## What this does to the rest of the system

- **The desktop stops writing drawings.** It serves, stores what devices send, and stamps. This
  tightens the bridge invariant in [ARCHITECTURE.md](../../ARCHITECTURE.md): the host owns the
  output directory and now never competes for it.
- **`mac-scratch` sheets already on disk stay.** `list_pages` reads the directory, so they keep
  appearing in the register as history. Nothing is deleted. `DESKTOP_SCRATCH_PAGE_ID` and the test
  pinning its value to `"mac-scratch"` both stay — a future writer must not reuse the name for
  something else.
- **`Drawing` stays in `src/drawing.rs`.** It is the shared wire model and the integration tests
  build snapshots with it. Only `app.rs`'s use of it goes.
- **`export::write_snapshot` loses its last caller in the binary.** The tests still use it through
  `#[path]` includes, so it needs `#[allow(dead_code)]` with a comment, or a caller. Do not delete
  it — `promote_page` and the tests depend on the shape.
- **No protocol, schema, or pairing change.** Nothing on the wire moves.

## Testing

`MobileServer::start_with_token` already takes a port, so port ownership is testable without
touching 8787:

- Two servers started on the same port: the first binds, the second reports that GoghMode already
  holds it.
- A foreign listener on the port: the second reports it as foreign rather than as ours.
- The classifier alone, against a body with and without `pairing-v2`.

The user interface itself has no automated coverage — egui has no practical snapshot testing here,
and pretending otherwise would be worse than saying so. It is verified by running it. What the suite
can hold is that removing the canvas breaks nothing: `tests/app_mobile_url.rs`, the prompt tests, and
the export and pages tests all keep passing.

## Deliberately not doing

- A configurable port. It reintroduces exactly the "which URL is current" problem this removes.
- A per-sheet `Copy image`. Decided against; the register stays a register.
- Touching the register table, the server protocol, or the pairing flow.
