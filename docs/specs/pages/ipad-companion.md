# iPad Companion (PencilKit)

> Bundle: `dev.goghmode.companion` · Source: `ipad-companion/GoghModeCompanion/` · Status: done

## Goal & user
The primary writing surface. Someone with an Apple Pencil who wants handwriting,
sketching, and whiteboarding to feel like Apple's own apps — low latency, pressure,
smoothing, palm rejection, a real eraser — and who wants the page to reach the Mac
without thinking about it.

Universal build: iPhone and iPad (`TARGETED_DEVICE_FAMILY = "1,2"`), iOS 17
minimum.

## Layout
Setup until paired, then a `NavigationStack` whose root is the register.

- **Setup** — the host list (`HostListView`). Pairing scans a QR code from the
  desktop's Devices panel, or takes the same payload pasted as text. The payload
  carries every address the host offered — the LAN address first, and the host's
  own `.local` name second, when that name is URL-safe — so a Wi-Fi move is a
  fallback to try before it is a re-pair. `HostStore` keeps the saved hosts and
  each host's address set; the keys are in the Keychain, non-syncing. Reachable
  again later as a settings sheet. The old URL field is gone: it could only create
  unauthenticated links, which [ADR-0006](../../decisions/0006-paired-devices-over-shared-url-token.md)
  retires. An endpoint saved by an earlier build is adopted into the list instead.
- **Register** (root, `RegisterView`) — the overview. Head rule naming the stamped
  sheet, then a ruled index: one line per sheet or series, columns `SHEET`, `NAME`,
  `UPDATED`, `STROKES`, `AGENT`. Toolbar carries **New sheet** and **Settings** —
  new sheets are made here and nowhere else. Dragging one line onto another files
  both into a series; a series line pushes `SeriesView`, the same index scoped to it.
- **Canvas** (`CanvasView`, pushed) — a full-bleed `PKCanvasView` with the system
  tool palette floating over it. Navigation title is the sheet's name; the back
  button returns to the register. Toolbar: status badge (also retry), stamp control,
  rename, clear. Leaving the sheet uploads it immediately rather than waiting out the
  debounce.
- **Keeping the screen awake** — on by default, user-toggleable from the host list.
  The idle timer is held only while the scene is active: the OS re-arms it the
  moment the app backgrounds, so the app re-decides on every phase change rather
  than holding a flag the OS has already ignored.

## Components

| File | Responsibility |
| --- | --- |
| `GoghModeCompanionApp.swift` | `@main`, single `WindowGroup`. |
| `ContentView.swift` | Pairing gate, the navigation stack, `CanvasView`, `StatusBadge`, `SetupView`. |
| `RegisterView.swift` | The overview: head rule, ruled index, rows, stamp control, series, previews. |
| `PageStore.swift` | Local pages and series, persistence, sheet numbering, recorded pin. |
| `DrawingSetStyle.swift` | The Drawing Set tokens and the shared drafting primitives. |
| `PencilCanvasView.swift` | `UIViewRepresentable` around the sheet: ruling behind, `PKCanvasView` and `PKToolPicker` on top. |
| `DrawingSnapshot.swift` | Codable wire schema and the `PKDrawing` → snapshot conversion. |
| `GoghModeClient.swift` | Endpoint normalization, `URLSession` POST, capabilities, pin/promote, `UploadError`. |
| `UploadController.swift` | `@MainActor ObservableObject` — debounce, status machine, retry, capability probe. |

### Sub-component specs
- [export-contract](../components/export-contract.md) — the schema this app must match exactly.
- [mobile-server-api](../components/mobile-server-api.md) — what the Mac accepts and why it rejects.

## Design tokens
The Drawing Set tokens in [`DESIGN.md`](../../../DESIGN.md), carried in
`DrawingSetStyle.swift`. The status dot is the one place colour alone still varies:
`review` blue for idle and saved, `ink-label` for waiting and saving, and red for
failed, wrong host, and re-pair, always beside a written label, never on its own.

That red is the one place the app spends a saturated colour outside the issue
stamp, which `DESIGN.md` otherwise forbids. It predates the drawing-set direction
and is a known breach rather than an exception the design grants: either the dot
loses its colour or `DESIGN.md` gains a second sanctioned use. Recorded so it is
decided rather than inherited.

## Tech used

**PencilKit setup**: the canvas is a window onto a fixed sheet, not a drawing area
the size of the view.

- **The surface is the view.** A fixed 1024 × 1366 portrait page was tried and
  reverted: it made the export one stable shape, but on a landscape iPad it left a
  portrait sheet with dead space beside it, both white, so the surface appeared to
  stop in the middle of the screen. `SheetPage.size` survives only as the stand-in
  for sending a sheet from the register, where no view has measured it.
- **Zoom runs from 1x to 4x.** The surface is the view at 1x and `contentSize`
  grows from there, so there is something to zoom into and nothing to zoom out to.
- **Ruling is drawn behind the canvas**, in `SheetRulingView`, with the canvas
  background clear. `PKCanvasViewDelegate` inherits `UIScrollViewDelegate`, so pan
  and zoom are reported and the rules stay pinned to the page rather than the
  screen. The ink matches the exporter's exactly, or the sheet on the iPad and the
  page the agent reads would be two different pages.
- **A stroke past the assumed page grows the exported canvas** rather than being
  clamped to it, which is what carries a sheet sent from the register, where the
  fallback size may be narrower than the surface it was drawn on.
- No bounce, `contentInsetAdjustmentBehavior = .never`.

- **Drawing policy is `.default`, not `.anyInput`** — a deliberate reversal of the
  original plan. `.default` honours the system pencil-only preference, so palm and
  finger taps stop leaving dots; the tool picker exposes a toggle for people
  drawing without a Pencil.
- **The tool picker is held on the Coordinator.** Releasing it takes the palette
  with it. `stateAutosaveName = "goghModeToolPicker"` persists tool, colour, and
  width across launches. Since `PKCanvasView` conforms to `PKToolPickerObserver`,
  `addObserver` + `setVisible(true, forFirstResponder:)` is all that is needed to
  get pen, **eraser**, lasso, colours, and widths — that is how the missing-eraser
  complaint was fixed.
- `becomeFirstResponder()` is called only once the view has a window, because the
  picker only appears for the first responder.
- **Clear is a monotonic `Int` signal**, compared against the coordinator's last
  seen value — not a boolean, which would need resetting and could be missed.

**Networking** — plain `URLSession`, one JSON POST, no chunking. `Info.plist`
carries `NSAllowsLocalNetworking` and `NSLocalNetworkUsageDescription`, both
required for plain-HTTP LAN traffic.

## Auth & access
Two kinds of saved host, and the interface says which is which.

**Paired** (`credential == .paired`). The device holds a key derived during
pairing, never received, kept in the Keychain as
`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` so it does not travel to
another device through a backup. Every upload is signed, and **the reply must be
signed back before the app reports success** — a machine that merely answers at
the saved address cannot pass for the paired host. Failing that check is its own
status, `wrongHost`, deliberately not merged into `Offline`, because "offline"
invites a retry and this must not be retried into.

A paired host stores **every** address the pairing offered, not just the one that
worked. An upload tries the active address first and falls back to the rest on
network-level failure only — never on a rejection, because a host that answers is
healthy and the problem is identity, not reachability. A success on a fallback
address is written back as the new active one, so the list learns where the host
actually is. When every saved address fails to answer, the state is a third
kind, `needsRepair`, presented as **Re-pair**: a retry cannot fix a dead address,
and the repair is one scan, because the identity was paired, not the address.

**Legacy** (`credential == .legacyURL`). The original secret URL, kept so an
endpoint saved by an older build is not stranded — it is adopted into the host
list on first launch. `GoghModeEndpoint` still requires http or https plus a host,
then appends `save` unless the path already ends in `/save`. Labelled
"unauthenticated link" in the list rather than dressed up.

No discovery, no Bonjour, no `NWBrowser`. Pairing carries the addresses — the LAN
one and the host's `.local` name. Carrying a name the client already holds is
data, not an mDNS implementation on either side; discovery would be the host
announcing itself so a stranger could find it, and that remains ruled out (see
[ADR-0004](../../decisions/0004-no-http-framework.md)).

## Data

`DrawingSnapshot` mirrors the Rust struct field for field, encoded with a plain
`JSONEncoder` and no key strategy — Swift property names *are* the wire names, and a
test asserts the key shape against the Rust side.

Conversion from `PKDrawing`:

- One `Stroke` per `PKStroke`, id `"stroke-{index+1}"` — index-based, so stable
  within a snapshot but not across edits.
- Points come from iterating `PKStrokePath` directly, not from distance-based
  interpolation (a second deliberate divergence from the plan).
- **Rounding happens before clamping**: x and y to hundredths, pressure to
  thousandths, *then* clamp to the canvas and to 0…1. The order matters — rounding
  after clamping can push an edge point just outside the canvas and earn a 400 from
  the Mac's validator. A test locks the order in.
- Full `Double` precision costs roughly 250 bytes per point on the wire and the Mac
  stores `f32` anyway, hence the rounding.
- `t` is `timeOffset * 1000 + pointIndex` — a monotonic tiebreaker, not a wall
  clock. (The web client sends epoch milliseconds; nothing downstream cares.)
- Stroke width is the mean of `max(size.width, size.height)` over the path, clamped
  1…80, defaulting to 4 on an empty path.
- Colour via `UIColor` hex, alpha discarded, `#111827` as fallback.
- Empty strokes dropped; background hardcoded `#ffffff`.
- `DrawingSnapshot.empty(canvasSize:)` is what **Clear** posts.

## Client state
`UploadController` is the state machine for the connection:
`idle · waiting · saving · saved · failed · wrongHost · needsRepair`. `.failed`
presents as **"Offline"** and `.needsRepair` as **"Re-pair"**, which opens the
pairing screen. It also remembers the last snapshot so a manual retry has something
to send.

`PageStore` owns everything about the sheets: pages, series, the recorded pin, each
sheet's ruling, and each sheet's recent states.

**Sheet history.** A sheet's last twenty states are kept in a sidecar beside the
page store, one file per sheet, so stepping back survives closing the sheet. The
canvas's own undo stack cannot: reopening a sheet builds a fresh `PKCanvasView`.
One state is recorded per finished stroke, which is what the stroke count changing
signals. Stepping back and then drawing abandons what was ahead. Restoring goes
through `PageStore.restore`, not `update`, because `update` refuses an empty drawing
for a sheet that has strokes and a sheet stepped back past its first stroke is
genuinely empty.

## Upload and retry

The whole drawing is re-sent 600 ms after every stroke — see Phase 4 in
[PLANNING.md](../../PLANNING.md). Six layers of resilience, each added for a
specific observed failure:

1. **Debounce** — `schedule()` cancels any pending task, sets `.waiting`, sleeps
   600 ms, then uploads. Cancellation is swallowed silently.
2. **One socket-level retry** — on `URLError` in `networkConnectionLost`,
   `timedOut`, or `cannotConnectToHost`, wait 300 ms and try once more. URLSession
   hands back a pooled socket the Mac already closed, which looks like a dead server
   but is not.
3. **Manual retry** — the status badge is a button, enabled when a failed upload has
   a remembered snapshot. Before this, nothing retried until the drawing changed and
   `Offline` stuck forever.
4. **Foreground retry** — `scenePhase == .active` triggers a retry if offline.
   Returning to the app is exactly when the Mac was most likely just reopened.
5. **Address fallback (paired hosts)** — the saved address set is tried in order
   when an address cannot be reached at the network level. A rejection never
   triggers a hop, for the reason above; and a hop that succeeds is recorded, so
   the active address tracks the Wi-Fi rather than drifting behind it. When every
   address is dead, the state becomes `.needsRepair` — the repair is a re-pair,
   presented as a tappable badge, not an `Offline` that retries into the wall.
6. **Learning a ruling refusal** — a 400 (or a named rejection) on a sheet that
   carried ruling means the host predates ruling, so the sheet is re-sent plain
   and the fact is remembered per address, until the host is forgotten, rather
   than complained about every stroke.

Errors map to actions, not codes:

| Condition | Message |
| --- | --- |
| Connection lost / timed out | Open GoghMode on the Mac, then tap to retry. |
| No internet | Join the same Wi-Fi as the Mac. |
| DNS / cannot find host (legacy link) | Copy the mobile URL from the Mac again. |
| Every address of a paired host dead | **Re-pair** — the Wi-Fi may have moved; the badge opens the pairing screen. |
| Non-2xx status | Reports the status code — the Mac's rejection reason is the body. |

## States

| State | Behaviour |
| --- | --- |
| Setup | Host list and pairing. No canvas until a host is selected. |
| Idle | Green dot, canvas ready. |
| Waiting | Orange dot during the 600 ms debounce. |
| Saving | Orange dot, request in flight. |
| Saved | Blue dot, with the time of the save in mono beside the label. |
| Offline (failed) | Red dot, tappable. The sentence is on the notice line, not in the chip. |
| Wrong host | Red dot, not tappable. Retrying into a machine that cannot prove itself is the thing to avoid. |
| Re-pair (needs repair) | Red dot, tappable, opens the pairing screen. Set only when a paired host's whole saved address set stopped answering. |
| Cleared | Canvas reset, clear signal bumped, an empty snapshot posted so the host's files match. Recorded on both sides of the erase, so it is one step back. |
| Ruled | The sheet carries a ruling; the snapshot goes as schema version 3 and the export carries the rules. |
| Ruling refused | The host predates ruling, so the sheet is re-sent plain and the notice line says why once. |

The status chip holds one shape in every state. Both its slots, the label and the
time, are reserved at their widest, so the bar it sits in does not jump. Anything
longer than a label belongs on the notice line.

On an open sheet the notice line is drawn over the canvas, never stacked above it.
A banner that takes layout space resizes the drawing surface the moment it appears,
which moves the paper under the pen in the middle of a line.

The sentence itself is read from `UploadController.complaint`, not from `status`.
A complaint is set when something actually fails and cleared only when a save or a
stamp succeeds, so it stays readable while the next attempt is already in flight.
Derived from `status` it flickered once per stroke, because every save passes
through `waiting` and `saving` before it can fail again. A cancelled upload is not
a failure at all: cancelling the in-flight request is how the next stroke replaces
the previous one, and `URLError.cancelled` is dropped rather than reported.

## Estimate
Shipped. Only remaining work is listed.

| Scope | Estimate |
| --- | --- |
| Canvas, tool picker, toolbar | shipped |
| Snapshot conversion + tests | shipped |
| Upload, debounce, retry layers | shipped |
| TestFlight pipeline | shipped |
| Page switcher (Phase 1) | shipped |
| Swipe to delete a sheet | shipped |
| Zoom and sheet history | shipped |
| Ruling, per sheet, baked into the export | shipped |
| QR pairing (Phase 2) | not estimated |
| Incremental upload (Phase 4) | not estimated |
| **Total** | — |

## Tasks
- [ ] Skip the upload when the drawing has not changed since the last successful one
      — the cheapest fix for the resend cost.
- [x] Replace URL paste with QR scanning. The preview follows the device through
      `AVCaptureDevice.RotationCoordinator`, so the camera and the tablet agree about
      which way is up. **Still to be run on a device**: pasting the payload remains
      the tested path.
- [x] A paired host whose addresses go stale. Pairing now carries the LAN address
      plus the host's own `.local` name; the companion falls back across the set,
      records which one answered, and when all of them die it offers a re-pair in
      one scan instead of a dead-end `Offline` — the identity already survives the
      move, so the repair is a scan, not a setup.
- [ ] Discover a moved host on its own. What the fallback cannot do is find a
      brand-new address nobody offered: the full interface list (the VPN case)
      remains open, and multicast discovery stays decided against — see Phase 5
      in [PLANNING.md](../../PLANNING.md).

## Open questions
- Should stroke ids survive edits? Page identity is now stable — a client-minted
  UUID, immutable for the life of the page — but stroke ids are still regenerated as
  `stroke-{n}` on every snapshot. Nothing downstream depends on them yet.
- ~~Should the app hold pages locally and switch between them, or mirror what the Mac
  holds?~~ **Answered: locally.** `PageStore` persists each page's `PKDrawing` to the
  app container and the overview renders thumbnails from those, so switching works
  with the Mac closed and no read endpoint was needed. Pages that live only on the
  Mac (`mac-scratch`, the browser companion's) are not visible here.
