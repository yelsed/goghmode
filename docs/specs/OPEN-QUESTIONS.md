# Open Questions

> A consolidated, themed buffer for unresolved decisions surfaced during spec work.
> **Answer once, then propagate** the answer back into the affected spec(s) and,
> if it's a significant choice, record it as an [ADR](../decisions/README.md).
> Keep resolved items here (moved to "Resolved") for the trail.

## Open

### Multiple pages and history
All five must be answered before Phase 1 in [PLANNING.md](../PLANNING.md) can start.
They are entangled — answering one in isolation forces the others.

- [ ] **Does `latest.*` keep meaning "the most recently touched page"?** Keeping it
      is what stops `/goghmode` and every other consumer from breaking. Strong
      default: yes, keep it and add history *alongside*. Affects
      [export-contract](components/export-contract.md).
- [ ] **Where does history live?** `drawings/pages/<id>/{json,svg,png}` plus an index
      file, versus dated filenames. An index is easier to list and harder to keep
      consistent. Affects [export-contract](components/export-contract.md).
- [ ] **Who owns page identity — iPad or Mac?** The iPad knows which page the user is
      on; the Mac owns the directory. The schema has no page identifier today, so
      `schemaVersion` goes to 2 and `validate_snapshot` moves with it. Affects all
      three page specs.
- [ ] **What does the overview actually show?** Thumbnails of pages held locally on
      the iPad, or a view of what the Mac holds? The second needs a read endpoint
      the server does not have. Affects
      [mobile-server-api](components/mobile-server-api.md) and
      [ipad-companion](pages/ipad-companion.md).
- [ ] **Deletion and renaming** — a write-only design never had to answer this.

### What the Mac app becomes
- [ ] Three options, leaning toward the second, decided once multi-page exists:
      keep both canvases and share the toolset · **demote the canvas and promote the
      bridge** (status, page browser, QR pairing, quick sketch fallback) · headless
      with a menu bar item. Affects [desktop-canvas](pages/desktop-canvas.md).

### Network exposure and pairing
- [ ] Should local-network access stay enabled by the secret URL alone, or should the
      Mac confirm a new device the first time it posts? Affects
      [mobile-server-api](components/mobile-server-api.md) and
      [ADR-0002](../decisions/0002-token-in-path-lan-pairing.md).
- [ ] Should the desktop expose the server by default while the window is open, or
      require explicit opt-in each session?
- [ ] Should the port fallback warn, or refuse and fail loudly? A QR code makes the
      question mostly moot.

### Schema and rendering divergences
- [ ] When `schemaVersion` goes to 2, does the Mac still accept version 1 uploads?
      Three client implementations cannot ship simultaneously.
- [ ] Should the PNG rasterizer honour `stroke.color`? Today it does not, so iPad
      colour survives only in the SVG and JSON. Affects
      [export-contract](components/export-contract.md).
- [ ] Should `canvas.background` be honoured on export or removed from the schema?
      Clients send it and nothing uses it.

### Scope questions carried from the vision document
- [ ] Are notebook and whiteboard separate modes, or one canvas with templates?
- [ ] Should Obsidian export be automatic or explicit, once it exists?

## Resolved

- [x] **QR code, short code, or manual URL entry for pairing?** → Manual URL entry,
      for now. Shipped as a text field on iPad and a Copy mobile URL button on the
      Mac. QR remains Phase 2 — it is the fix that makes the stale-port trap
      irrelevant rather than merely visible. Propagated to
      [ipad-companion](pages/ipad-companion.md) and
      [desktop-canvas](pages/desktop-canvas.md).
- [x] **Should clients send vector strokes, PNG snapshots, or both?** → Vector
      strokes. The Mac owns rendering, so strokes stay re-exportable. Propagated to
      [export-contract](components/export-contract.md); see
      [ADR-0003](../decisions/0003-vector-strokes-over-png-upload.md).
- [x] **Is the first native app iPad-only or universal?** → Universal
      (`TARGETED_DEVICE_FAMILY = "1,2"`), iOS 17 minimum. Propagated to
      [ipad-companion](pages/ipad-companion.md).
- [x] **Which Apple signing route?** → TestFlight through GitHub Actions, not cable
      install. Propagated to the release section of [ARCHITECTURE.md](../ARCHITECTURE.md).
- [x] **`PKCanvasView` drawing policy: `.anyInput` or `.default`?** → `.default`,
      reversing the original plan. It honours the system pencil-only preference so
      palm and finger taps stop leaving dots, and the tool picker offers a toggle for
      people without a Pencil. Propagated to [ipad-companion](pages/ipad-companion.md).
- [x] **Interpolate `PKStrokePath` by distance, or iterate it directly?** → Iterate
      directly. Simpler, and the point count is already acceptable after rounding.
      Propagated to [ipad-companion](pages/ipad-companion.md).
- [x] **Round coordinates before or after clamping?** → Before. Rounding afterwards
      can push an edge point outside the canvas and earn a 400 from
      `validate_snapshot`. Locked in by a Swift test; propagated to
      [ipad-companion](pages/ipad-companion.md).
