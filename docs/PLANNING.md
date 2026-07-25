# Build Planning — GoghMode

> **Audience:** PM / planning. What is next and what gates what. This is a solo
> project, so phases are **ordered, not scheduled** — there are no dates and no
> estimates, and nothing here is a commitment.

The capture-to-agent loop works end to end on all three surfaces. Everything below
is deferred product work, parked until that loop is boring and reliable. Source:
[`later.md`](../later.md) (25 July 2026) and the open items in
[mobile-companion-roadmap.md](mobile-companion-roadmap.md).

## Dependencies

| What | Gates | Why |
| --- | --- | --- |
| Multi-page storage (Phase 1) | Mac role rework (Phase 3) | A page browser is the main thing that would make the Mac window worth opening. Deciding the Mac's role first would be guessing. |
| Multi-page storage (Phase 1) | Incremental upload (Phase 4) | Full resends only hurt on long-lived pages, which is exactly what multi-page creates. |
| Page identity in the schema | Everything in Phase 1 | `schemaVersion` goes to 2, and `validate_snapshot` plus all three client implementations move together. See the wire contract in [ARCHITECTURE.md](ARCHITECTURE.md). |
| Answers to the five multi-page questions | Starting Phase 1 | Listed under **Open** in [OPEN-QUESTIONS](specs/OPEN-QUESTIONS.md). |

## Phases

### Phase 1 — Multiple pages and a notes overview
The highest-value item. Doubt about whether written work survives is enough to stop
someone writing.

- [x] Decide the five open questions in [OPEN-QUESTIONS](specs/OPEN-QUESTIONS.md)
      (does `latest.*` keep its meaning, where history lives, who owns page
      identity, what the overview shows, deletion and renaming).
- [x] Add a page identifier to `DrawingSnapshot`. `schemaVersion` goes to 2, but
      `validate_snapshot` accepts `{1, 2}` rather than bumping — refusing version 1
      would break every installed companion. See
      [export-contract](specs/components/export-contract.md).
- [x] Update all three clients to send the new field:
      [desktop-canvas](specs/pages/desktop-canvas.md),
      [mobile-web-canvas](specs/pages/mobile-web-canvas.md),
      [ipad-companion](specs/pages/ipad-companion.md).
- [x] Mac writes page history **alongside an unchanged `latest.*`**, so `/goghmode`
      and every other consumer keeps working.
- [x] iPad gets a page switcher over locally-held pages.
- [x] Mac-side browsing, brought forward rather than deferred: the Mac owns the
      directory and reads it directly, so no read endpoint was needed.
- [ ] Deletion and renaming. Deliberately outside the first slice; when it lands it
      moves pages to `pages/.trash/<id>/` rather than unlinking, so undo is a rename.
- [ ] Retention. Pages are kept forever and nothing expires them. ~40 KB per page,
      overwritten in place, so there is no storage pressure to manage — and deleting
      handwritten notes on a timer would re-create the fear this phase removes.

### Phase 2 — Pairing without copy-paste
Independent of Phase 1; can be done at any time.

- [ ] Show a QR code for the mobile URL on the Mac.
- [ ] Handle the ephemeral-port fallback: at minimum warn when the server did not
      get 8787, since a stale URL currently fails silently. See **Known failure
      modes** in [ARCHITECTURE.md](ARCHITECTURE.md).
- [ ] Decide whether the secret URL alone stays sufficient or the Mac should
      confirm a new device — see [ADR-0002](decisions/0002-token-in-path-lan-pairing.md).

### Phase 3 — What the Mac app becomes
Gated on Phase 1. The feedback was "the Mac app feels a bit redundant", but the Mac
is the only thing that owns the drawings directory, runs the server, and holds the
token — so "make them equal" is the wrong frame. Three options, leaning toward the
second:

- [ ] Keep both canvases and share the toolset — most literal, most work, and it
      argues against PencilKit being the point of the native app.
- [ ] **Demote the canvas, promote the bridge** — connection status, page browser,
      QR pairing, with a quick sketch canvas kept for when no iPad is nearby.
- [ ] Headless with a menu bar item — smallest surface, loses the fallback canvas,
      makes pairing awkward.

### Phase 4 — Stop resending the whole drawing
The iPad posts the entire drawing 600 ms after every stroke. Rounding coordinates
cut the payload a lot, but cost still grows with page length. Cheapest first:

- [ ] Skip the upload when nothing changed since the last successful one.
- [ ] Send only strokes added since the last acknowledged upload, with the Mac
      appending — needs per-session state the Mac does not keep today.
- [ ] Full snapshots on explicit save only, deltas on autosave.

### Later / unscheduled
- [ ] Photo and snapshot import, so a phone photo of a paper page enters the same
      bridge.
- [ ] Obsidian export into the LLM wiki vault — a storage and review layer, added
      only once the capture loop is excellent.
- [ ] PNG upload path for clients that can only send raster images.
- [ ] Rust CI. `cargo test` currently runs locally only.

## Notes

- No estimates, no milestones, no target dates — this is not tracked in an external
  planning tool either.
- Phases 2 and 4 are small and self-contained; Phases 1 and 3 are the ones that
  change the product's shape.
- Bug fixes are not planned here. The three issues from the first round of iPad
  feedback — no eraser, sticky `Offline` status, unhelpful error text — are already
  fixed.
