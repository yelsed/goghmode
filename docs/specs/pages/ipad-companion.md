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
  desktop's Devices panel, or takes the same payload pasted as text. `HostStore`
  keeps the saved hosts; their keys are in the Keychain, non-syncing. Reachable
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

## Components

| File | Responsibility |
| --- | --- |
| `GoghModeCompanionApp.swift` | `@main`, single `WindowGroup`. |
| `ContentView.swift` | Pairing gate, the navigation stack, `CanvasView`, `StatusBadge`, `SetupView`. |
| `RegisterView.swift` | The overview: head rule, ruled index, rows, stamp control, series, previews. |
| `PageStore.swift` | Local pages and series, persistence, sheet numbering, recorded pin. |
| `DrawingSetStyle.swift` | The Drawing Set tokens and the shared drafting primitives. |
| `PencilCanvasView.swift` | `UIViewRepresentable` around `PKCanvasView` + `PKToolPicker`. |
| `DrawingSnapshot.swift` | Codable wire schema and the `PKDrawing` → snapshot conversion. |
| `GoghModeClient.swift` | Endpoint normalization, `URLSession` POST, capabilities, pin/promote, `UploadError`. |
| `UploadController.swift` | `@MainActor ObservableObject` — debounce, status machine, retry, capability probe. |

### Sub-component specs
- [export-contract](../components/export-contract.md) — the schema this app must match exactly.
- [mobile-server-api](../components/mobile-server-api.md) — what the Mac accepts and why it rejects.

## Design tokens
Native SwiftUI defaults. The only colour decisions are the status dot: green for
idle and saved, orange for waiting and saving, red for failed.

## Tech used

**PencilKit setup** — zoom pinned to 1×, no bounce, `contentInsetAdjustmentBehavior = .never`.

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

**Legacy** (`credential == .legacyURL`). The original secret URL, kept so an
endpoint saved by an older build is not stranded — it is adopted into the host
list on first launch. `GoghModeEndpoint` still requires http or https plus a host,
then appends `save` unless the path already ends in `/save`. Labelled
"unauthenticated link" in the list rather than dressed up.

No discovery, no Bonjour, no `NWBrowser`. Pairing carries the address.

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
`UploadController` is the only state machine: `idle · waiting · saving · saved · failed`.
`.failed` presents as **"Offline"**. It also remembers the last snapshot so a manual
retry has something to send.

## Upload and retry

The whole drawing is re-sent 600 ms after every stroke — see Phase 4 in
[PLANNING.md](../../PLANNING.md). Four layers of resilience, each added for a
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

Errors map to actions, not codes:

| Condition | Message |
| --- | --- |
| Connection lost / timed out | Open GoghMode on the Mac, then tap to retry. |
| No internet | Join the same Wi-Fi as the Mac. |
| DNS / cannot find host | Copy the mobile URL from the Mac again. |
| Non-2xx status | Reports the status code — the Mac's rejection reason is the body. |

## States

| State | Behaviour |
| --- | --- |
| Setup | Host list and pairing. No canvas until a host is selected. |
| Idle | Green dot, canvas ready. |
| Waiting | Orange dot during the 600 ms debounce. |
| Saving | Orange dot, request in flight. |
| Saved | Green dot. |
| Offline (failed) | Red dot, tappable, with the guidance above. |
| Cleared | Canvas reset, clear signal bumped, an empty snapshot posted so the Mac's files match. |

## Estimate
Shipped. Only remaining work is listed.

| Scope | Estimate |
| --- | --- |
| Canvas, tool picker, toolbar | shipped |
| Snapshot conversion + tests | shipped |
| Upload, debounce, retry layers | shipped |
| TestFlight pipeline | shipped |
| Page switcher (Phase 1) | not estimated — see [PLANNING.md](../../PLANNING.md) |
| QR pairing (Phase 2) | not estimated |
| Incremental upload (Phase 4) | not estimated |
| **Total** | — |

## Tasks
- [ ] Skip the upload when the drawing has not changed since the last successful one
      — the cheapest fix for the resend cost.
- [x] Replace URL paste with QR scanning. **The scanner compiles but has not been
      run on a device** — pasting the payload is the tested path.
- [ ] Update a paired host's address when it moves, rather than needing a re-pair.
      The identity already survives the move; only the stored address does not.

## Open questions
- Should stroke ids survive edits? Page identity is now stable — a client-minted
  UUID, immutable for the life of the page — but stroke ids are still regenerated as
  `stroke-{n}` on every snapshot. Nothing downstream depends on them yet.
- ~~Should the app hold pages locally and switch between them, or mirror what the Mac
  holds?~~ **Answered: locally.** `PageStore` persists each page's `PKDrawing` to the
  app container and the overview renders thumbnails from those, so switching works
  with the Mac closed and no read endpoint was needed. Pages that live only on the
  Mac (`mac-scratch`, the browser companion's) are not visible here.
