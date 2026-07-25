# Open Questions

> A consolidated, themed buffer for unresolved decisions surfaced during spec work.
> **Answer once, then propagate** the answer back into the affected spec(s) and,
> if it's a significant choice, record it as an [ADR](../decisions/README.md).
> Keep resolved items here (moved to "Resolved") for the trail.

## Open

### Multiple pages and history
Four of the five are answered; see **Resolved**. What remains:

- [ ] **Deletion and renaming** — a write-only design never had to answer this. The
      layout leaves room: deletion moves a page to `pages/.trash/<id>/` rather than
      unlinking, which puts it one level deeper than the `pages/*/page.json` glob the
      index is rebuilt from, so a trashed page drops out without a filter.
- [ ] **Retention.** Shipped as "kept forever, nothing expires". If a gate is ever
      added it should be opt-in, default forever, recoverable, and never sweep a page
      the user promoted or renamed.

### What the Mac app becomes
- [x] **Decided: demote the canvas, promote the bridge.** Not on taste — `src/app.rs`
      was already writing `latest.*` on every stroke end, racing the iPad for the
      agent's only input file, so once every writer owns a page the Mac canvas is
      structurally just another source. It now writes `mac-scratch` and gained a page
      browser, a port warning, and a reveal-folder button; the quick sketch canvas
      stays. QR pairing is still Phase 2. Affects
      [desktop-canvas](pages/desktop-canvas.md).

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
- [ ] Should `canvas.background` be honoured on export or removed from the schema?
      Clients send it and nothing uses it.

### Scope questions carried from the vision document
- [ ] Are notebook and whiteboard separate modes, or one canvas with templates?
- [ ] Should Obsidian export be automatic or explicit, once it exists?

## Resolved

- [x] **Does `latest.*` keep meaning "the most recently touched page"?** → Yes, and
      it is a byte-identical mirror of the page written last. Forced rather than
      chosen: `src/skill.rs` is written to `~/.claude/skills/` only when the user runs
      `install-skill`, so installed copies can never be updated in step with this app.
      Propagated to [export-contract](components/export-contract.md).
- [x] **Where does history live?** → `drawings/pages/<id>/page.{json,svg,png}` plus
      `pages/index.json`, and the index is **rebuilt by directory scan** after every
      write rather than maintained incrementally, so it cannot drift and needs no
      repair path. Dated filenames were rejected because they encode identity in the
      name, and the iPad re-uploads a whole page on every stroke — a stable directory
      makes that an idempotent overwrite. Propagated to
      [export-contract](components/export-contract.md).
- [x] **Who owns page identity — iPad or Mac?** → The client that created the page
      mints the id. The iPad works offline and retries uploads, so a Mac-minted id
      would leave an offline page unnameable and make retries duplicate-prone. The id
      becomes a directory name, so the server treats it as untrusted input and
      restricts it to `[A-Za-z0-9_-]{1,64}`. Propagated to all three page specs.
- [x] **What does the overview actually show?** → Both, without a read endpoint. The
      iPad lists its own local pages; the Mac reads its own directory directly. No
      server-side listing was needed. Propagated to
      [ipad-companion](pages/ipad-companion.md) and
      [desktop-canvas](pages/desktop-canvas.md).
- [x] **When `schemaVersion` goes to 2, does the Mac still accept version 1 uploads?**
      → Yes. The server accepts `{1, 2}` and files version 1 under a reserved `legacy`
      page. A bare bump would have bricked every installed companion build. A new
      `GET {prefix}capabilities` lets a client ask what a Mac takes; an older Mac
      404s that route, which the iPad reads as "version 1 only". Propagated to
      [export-contract](components/export-contract.md) and
      [mobile-server-api](components/mobile-server-api.md).
- [x] **Should the PNG rasterizer honour `stroke.color`?** → Yes. It hardcoded
      `#111827` while the SVG honoured the colour, so every thumbnail of a colour
      drawing rendered black — which only became visible once the Mac grew a page
      browser. Propagated to [export-contract](components/export-contract.md).
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
