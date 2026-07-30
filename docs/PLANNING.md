# Build Planning — GoghMode

> **Audience:** PM / planning. What is next, in what order, and what gates what.
> The capture-to-agent loop is shipped; everything below is about making it worth
> living in. Deferred items and their full reasoning live in
> [`later.md`](../later.md).

## Dependencies (parallel tracks)

- **Multi-page storage gates almost everything else.** A page browser on the Mac,
  thumbnails, and delta uploads all assume pages exist. Decide the storage layout
  and the `schemaVersion` bump first — see [`later.md`](../later.md) item 1.
- **The `latest.*` contract gates the storage layout.** Whatever pages look like,
  `latest.*` has to keep meaning "most recently touched page" or `/goghmode` and
  every other consumer breaks — [ADR-0001](decisions/0001-latest-files-contract.md).
- **The Mac app's future role gates its UI work.** Do not redesign the Mac window
  before deciding whether it stays a canvas or becomes a bridge and page browser
  ([`later.md`](../later.md) item 2).
- **Nothing external gates any of this.** No third-party service, no design
  hand-off, no store review — the iPad app is installed from Xcode.

## Milestones

| Milestone | Target | Covers |
|---|---|---|
| M0 — Capture loop works | ✅ done | Mac sketchpad, three-file export, `/goghmode` skill, mobile web page, iPad companion, app bundle |
| M1 — Written work survives | next | Page identity in the snapshot, history on the Mac beside an unchanged `latest.*`, page switcher on the iPad |
| M2 — Pairing stops being fragile | after M1 | QR code for the mobile URL, loud failure when port 8787 is taken |
| M3 — The Mac window earns its place | after M1 | Decide bridge vs canvas, then build to that decision (likely a page browser) |
| M4 — Uploads stop resending everything | with/after M1 | Skip unchanged uploads first; deltas only if long pages prove it necessary |

## Phases

### Phase 1 — Page identity (M1)
- [ ] Add a page identifier to `DrawingSnapshot`, bump `schemaVersion` to 2, and
      extend `validate_snapshot` to match (`src/drawing.rs`, `src/mobile_server.rs`).
- [ ] Accept both version 1 and version 2 uploads for one release so an
      un-updated iPad keeps working.
- [ ] Write history alongside `latest.*` (layout to be decided: `pages/<id>/…`
      plus an index, versus dated filenames) in `src/export.rs`.
- [ ] Keep `latest.*` pointing at the most recently touched page, and cover it
      with a test that fails if it ever stops.

### Phase 2 — Pages on the iPad (M1)
- [ ] Local page list with a switcher; each page holds its own `PKDrawing`.
- [ ] Uploads carry the active page identifier.
- [ ] Decide what happens on delete and rename — the write-only design never had
      to answer this.

### Phase 3 — Pairing (M2)
- [ ] Show a QR code for the mobile URL in the Mac toolbar.
- [ ] Say so loudly when the server could not get port 8787, instead of silently
      falling back to a random port and leaving a stale iPad URL pointing at
      nothing.

### Phase 4 — Mac app's role (M3)
- [ ] Decide: keep both canvases, demote the canvas and promote the bridge, or go
      menu-bar-only. Record the choice as an ADR.
- [ ] Build to that decision — most likely a page browser over the history from
      Phase 1, which needs a read endpoint the server does not have yet.

### Phase 5 — Upload cost (M4)
- [ ] Skip the upload when the drawing has not changed since the last successful
      one (cheapest, no protocol change).
- [ ] Only if long pages still hurt: send strokes added since the last
      acknowledged upload and have the Mac append.

## Notes

- No dates. This is a personal tool built in evenings; the order matters, the
  calendar does not.
- Phases 1 and 2 are one feature split across two codebases and should land
  together behind the same schema version.
- Not counted anywhere above: Xcode signing churn, macOS permission prompts, and
  the time spent re-pairing devices while testing — historically the bulk of an
  iPad-side session.
- Bug fixes jump the queue. The loop being boring and reliable is the thing that
  makes the deferred features worth building at all.
