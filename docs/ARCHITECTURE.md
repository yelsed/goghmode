# Architecture

> **Audience:** developers. How the system fits together — read before
> contributing. Update this when a **structural pattern** changes (not for
> per-screen detail — that lives in [specs/](specs/README.md)).

## Stack

**Mac app and CLI — Rust** (`src/`, one binary called `goghmode`)
- `eframe` / `egui` 0.34 — immediate-mode GUI; one crate gives a native window, a
  canvas, and widgets with no platform UI code.
- `clap` — the `prompt`, `install-skill`, and `install-app` subcommands.
- `image` — rasterises strokes to PNG without a headless renderer.
- `serde` / `serde_json` — the `DrawingSnapshot` wire and file format.
- `arboard` — clipboard for `Copy image`.
- `home` — resolves `~` for the drawings directory, the skill path, and the token.
- The local HTTP server is hand-written on `std::net` — see
  [ADR-0002](decisions/0002-token-in-url-local-server.md).

**Phone / tablet web app — plain HTML, CSS, and JavaScript** (`mobile/`)
- No framework, no build step. The files are `include_bytes!`-embedded into the
  Rust binary, so the app ships as one executable.

**iPad companion — Swift / SwiftUI** (`ipad-companion/`)
- PencilKit for the canvas (`PKCanvasView`, `PKToolPicker`), `URLSession` for
  uploads. See [ADR-0003](decisions/0003-native-ipad-companion.md).

## System diagram

```text
   ┌───────────────────────┐          ┌──────────────────────────┐
   │  iPad companion       │          │  Phone / tablet browser  │
   │  (SwiftUI+PencilKit)  │          │  (mobile/index.html)     │
   └───────────┬───────────┘          └────────────┬─────────────┘
               │  POST /<token>/save               │
               │  DrawingSnapshot JSON             │
               └──────────────┬────────────────────┘
                              ▼
              ┌──────────────────────────────────┐
              │  Mac app (goghmode)              │
              │  ┌────────────────────────────┐  │
              │  │ app.rs — egui window       │  │
              │  │ mobile_server.rs — HTTP    │  │
              │  │ drawing.rs — stroke model  │  │
              │  │ export.rs — PNG/SVG/JSON   │  │
              │  └────────────────────────────┘  │
              └────────────────┬─────────────────┘
                               ▼
              ~/Pictures/GoghMode/drawings/latest.{png,svg,json}
                               ▲
                               │ reads
              ┌────────────────┴─────────────────┐
              │  Claude Code / other AI terminal │
              │  via /goghmode or prompt text    │
              └──────────────────────────────────┘
```

The Mac app is the only writer of the drawings directory, and the files are the
only interface the AI side has. Nothing talks to the internet.

## Folder structure

- `src/` — the Rust binary. One module per concern; see the module kit below.
- `mobile/` — the phone web app (`index.html`, `service-worker.js`,
  `manifest.webmanifest`, `icon.svg`), embedded into the binary at compile time.
  Editing these means rebuilding.
- `ipad-companion/` — Xcode project for the native iPad app, plus its tests.
- `tests/` — Rust integration tests, one file per surface.
- `docs/` — this documentation set; `docs/decisions/` holds the ADRs.
- `drawings/` — sample output only. Real output goes to `~/Pictures/GoghMode/`.

## Shared component / module kit

Reuse these before adding a new module:

- `drawing.rs` — `Drawing` (mutable in-memory document: strokes, active stroke,
  undo, clear) and the serialisable `DrawingSnapshot` / `Stroke` / `Point` /
  `CanvasSize` types. `Drawing::accepts_point` is the single bounds/finite guard.
- `export.rs` — `write_snapshot` (the only function that writes the three files),
  plus `snapshot_to_svg` and `snapshot_to_rgba` for callers that want one format.
- `mobile_server.rs` — `MobileServer::start` (bind, token, background thread) and
  `validate_snapshot` (the trust boundary for uploaded drawings).
- `prompt.rs` — the two prompt strings, deliberately free of shell
  metacharacters.
- `skill.rs` / `app_install.rs` — installers for the Claude skill and the macOS
  app bundle.
- `app.rs` — egui window: `draw_toolbar`, `draw_canvas`, `StatusBar`,
  `configure_visuals`, `primary_button`. Colors are inline `Color32` literals
  here; the chrome color (18, 24, 34) is mirrored in the iPad app as
  `Color.goghChrome`.

## Data flow (reading & writing)

One direction only: **stroke → snapshot → three files**. There is no read-back
endpoint and no cache to invalidate.

1. A pointer drag on any of the three canvases appends points to an in-memory
   drawing. Out-of-bounds and non-finite points are dropped at the source.
2. Local saves (Mac) call `Drawing::snapshot()` then `export::write_snapshot`.
   Remote saves (web, iPad) `POST` the snapshot JSON to `/<token>/save`, which
   validates it and calls the same `write_snapshot`.
3. `write_snapshot` writes `latest.json.tmp`, `latest.svg.tmp`, `latest.png.tmp`
   and renames each into place, so a crash mid-write leaves the previous drawing
   intact.
4. The agent reads `latest.*` — the contract in
   [ADR-0001](decisions/0001-latest-files-contract.md).

Autosave: the Mac app saves on `drag_stopped`; the iPad debounces 600 ms after
the last stroke change; the web app saves when you tap `Send to Mac`.

## Loading / empty / error convention

- **Mac:** a one-line status bar under the canvas is the only feedback channel.
  Every action sets it, including failures.
- **iPad:** one status badge — Ready / Waiting / Saving / Saved / Offline.
  Failures put the reason next to the badge and the badge becomes a retry
  button; the app also retries when it returns to the foreground.
- **Web:** the `#status` paragraph carries the same messages.
- **Server:** a rejected upload answers with the specific reason as plain text
  (`stroke 12 width 900 is outside 0.5-80`), never a bare `400`. Anything the
  user could act on has to be in that string.
- Nothing has a loading state worth designing: the only slow path is the upload,
  which is covered by `Saving`.

## Auth

There are no accounts. Access control is one shared secret in the URL path:

- On first run, `MobileServer` generates 16 random bytes from `/dev/urandom` and
  stores the hex in `~/.goghmode/mobile-token`; every later run reuses it, so
  home-screen shortcuts keep working.
- Every route lives under `/<token>/`. Without the token you get `404`.
- The server binds `0.0.0.0` and is reachable only from the local network, only
  while the Mac window is open.
- `POST /<token>/save` is the only mutating route. It accepts a `DrawingSnapshot`
  and writes only into the configured drawings directory — no path from the
  request reaches the filesystem.
- `validate_snapshot` is the trust boundary: schema version, canvas 1–4096,
  ≤ 4096 strokes, ≤ 200 000 points, stroke width 0.5–80, finite in-bounds
  coordinates, bounded colour/id strings, and a 4 MB body cap.

## State management — the rule

**The files on disk are the shared state; everything else is a local draft.**
Each canvas owns its own strokes and never reads back from the Mac, so there is
nothing to sync or reconcile — the last writer of `latest.*` wins. Any feature
that needs the Mac to hand state back (a page browser, thumbnails) is a new read
endpoint and a real design decision, not an incremental change.

## Conventions

- **Descriptive names, no single letters.** `canvas_rect`, `drawings_dir`,
  `point_count` — not `r`, `d`, `n`.
- **Comments explain why, not what.** The ones in the codebase mark traps that
  cost someone an afternoon (non-blocking sockets on macOS, a released
  `PKToolPicker` taking the palette with it, `run_and_return`). Match that bar.
- **Errors name the thing that is wrong**, with the offending value in the
  message.
- **The three-file contract is load-bearing.** Anything that writes drawings goes
  through `export::write_snapshot`; do not add a parallel writer.
- **Rust:** `cargo fmt` and `cargo clippy` clean. Tests live in `tests/` for
  behaviour that spans modules, and in a `#[cfg(test)] mod tests` block for
  module-local rules.
- **The chrome colour is a boundary, not decoration.** Toolbars are dark
  (18, 24, 34) precisely because the canvas is near-white paper — people try to
  draw on any surface that looks like paper. Keep new chrome obviously not-paper.

## Testing

```bash
cargo test                                    # Rust unit + integration tests
cargo clippy --all-targets                    # lints
xcodebuild -project ipad-companion/GoghModeCompanion.xcodeproj \
  -scheme GoghModeCompanion \
  -destination 'generic/platform=iOS Simulator' build   # iPad app compiles
```

Integration coverage in `tests/`: `mobile_server.rs` (routing, token rejection,
multi-packet uploads, snapshot validation), `mobile_web_assets.rs` (the embedded
assets are served and stay in sync), `export_snapshot.rs` (SVG/PNG/JSON output),
`prompt.rs` (prompt text stays paste-safe), `skill_install.rs`, `app_install.rs`,
`app_mobile_url.rs`. The iPad app has `DrawingSnapshotTests` for the
PencilKit → snapshot conversion.

"Done" means: tests pass, clippy is clean, and anything that changed the drawing
pipeline was checked end-to-end by drawing once and reading the resulting
`latest.*`.

## Related
- [OVERVIEW.md](OVERVIEW.md) · [PLANNING.md](PLANNING.md) · [specs/](specs/README.md) · [decisions/](decisions/README.md)
