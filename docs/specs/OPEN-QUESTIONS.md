# Open Questions

> A consolidated, themed buffer for unresolved decisions. **Answer once, then
> propagate** the answer back into the affected docs and, if it's a significant
> choice, record it as an [ADR](../decisions/README.md). Resolved items stay here
> for the trail.

## Open

### Multiple pages
- [ ] Where does history live — `drawings/pages/<id>/{json,svg,png}` plus an index
      file, or dated filenames? An index is easier to list and harder to keep
      consistent. Blocks [PLANNING.md](../PLANNING.md) Phase 1.
- [ ] Who owns page identity, the iPad or the Mac? The iPad knows which page the
      user is on; the Mac owns the directory. The snapshot has no page field
      today, so either answer means `schemaVersion` 2.
- [ ] What does the overview show — thumbnails of pages held locally on the iPad,
      or what the Mac holds? The second needs a read endpoint the server does not
      have; it only serves static assets and accepts `POST /<token>/save`.
- [ ] What do delete and rename do? A write-only design never had to answer this.
- [ ] Are notebook and whiteboard separate modes, or one canvas with templates?
      Raised in [ai-field-notebook-vision.md](../ai-field-notebook-vision.md).

### Pairing and connection
- [ ] QR code, short code, or manual URL entry as the primary pairing path?
      QR makes the stale-URL problem irrelevant rather than merely visible.
- [ ] When port 8787 is taken, warn and continue on a random port, or refuse to
      start? Today it falls back silently and a previously-copied iPad URL points
      at nothing, which looks exactly like a dead Mac app.
- [ ] Should local-network writes stay enabled by the secret URL alone, or need
      an explicit per-session confirmation in the Mac app?

### The Mac app's role
- [ ] Once the iPad is the good drawing surface, is the Mac window a canvas, a
      bridge with a page browser, or a menu bar item? Leaning bridge. Decide
      after pages exist — see [`later.md`](../../later.md) item 2.

### Upload cost
- [ ] Skip unchanged uploads, send deltas, or keep full snapshots on explicit
      save only? Cheapest first; not urgent while pages stay short.

### Formats
- [ ] Accept a PNG-only upload path for clients that cannot send strokes? Would
      mean `latest.svg` and `latest.json` no longer always agree with
      `latest.png`.

## Resolved

- [x] How does the AI side get the drawing? → Three files, always overwritten, in
      one fixed directory. See [ADR-0001](../decisions/0001-latest-files-contract.md)
      (2026-06-11).
- [x] How do phones reach the Mac without a cloud service? → A local HTTP server
      behind a persistent secret URL path, running only while the app is open.
      See [ADR-0002](../decisions/0002-token-in-url-local-server.md) (2026-06-12).
- [x] Is the mobile web page enough for the iPad? → No. Palm rejection, eraser,
      and pencil feel need PencilKit, so a native companion ships alongside it.
      See [ADR-0003](../decisions/0003-native-ipad-companion.md) (2026-07-25).
- [x] Does the drawings directory follow the terminal's working directory? → No.
      It is always `~/Pictures/GoghMode/drawings/` unless `--drawings-dir` says
      otherwise, because per-terminal histories meant `/goghmode` could read a
      drawing from an unrelated session (2026-07-27).
- [x] Should the mobile page send vector strokes or PNG? → Strokes, as a
      `DrawingSnapshot`, written through the same export module as the Mac canvas
      so there is only one file writer (2026-06-12).
