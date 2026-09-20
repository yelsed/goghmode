# Export Contract

> Source: `src/drawing.rs`, `src/export.rs`, `src/timeline.rs` · Used on: [desktop-bridge](../pages/desktop-bridge.md), [mobile-web-canvas](../pages/mobile-web-canvas.md), [ipad-companion](../pages/ipad-companion.md) · Status: done

## Purpose
The single definition of what a drawing *is* and what the Mac writes to disk. Every
drawing surface serializes to this shape, and every write goes through one function,
so the agent reading `drawings/latest.*` never has to care which device drew the
page. See [ADR-0001](../../decisions/0001-drawings-latest-as-the-agent-contract.md).

## Anatomy

### `DrawingSnapshot` — the wire format
```jsonc
{
  "schemaVersion": 4,
  "page": { "id": "9F2C4A1B", "title": "Server sketch" },
  "canvas": {
    "width": 1024.0, "height": 1366.0, "background": "#ffffff",
    "ruling": { "style": "grid", "spacing": 32.0 }
  },
  "strokes": [
    {
      "id": "stroke-1",
      "color": "#111827",
      "width": 4.0,
      "startedAt": 1758290001500,
      "points": [ { "x": 12.5, "y": 40.0, "pressure": 0.5, "t": 0 } ]
    }
  ],
  "narration": {
    "language": "nl",
    "engine": "whisperkit/openai_whisper-large-v3-v20240930_626MB",
    "segments": [ { "start": 1758290000000, "end": 1758290003400, "text": "Dit is de database." } ]
  }
}
```

| Field | Type | Notes |
| --- | --- | --- |
| `schemaVersion` | `u8` | `1`, `2`, `3` or `4`. A plain sheet still goes as `2`, so nothing changed for anyone not using ruling; `3` is sent only by a sheet that carries ruling, `4` only by one that carries narration. `1` is still accepted so installed clients keep working. `#[serde(rename)]` on the Rust side; the Swift and JavaScript clients spell it out. |
| `page.id` | `String` | Required at version 2, absent at version 1. Minted by the client that created the page and immutable after. Becomes a directory name, so the server restricts it to `[A-Za-z0-9_-]{1,64}`. |
| `page.title` | `String?` | Optional label for the overview. Up to 200 characters. |
| `canvas.width` / `.height` | `f32` | The page. The iPad sends a fixed 1024 × 1366 sheet, grown to cover anything drawn past it. The web client still sends its live surface size. |
| `canvas.background` | `String` | Sent by clients, **ignored on export**: both writers paint white. Kept in the schema rather than removed, because two shipped clients send it. |
| `canvas.ruling` | `Ruling?` | Absent on a plain sheet, which is every sheet unless someone chose otherwise. See [ADR-0007](../../decisions/0007-ruling-is-a-writing-aid-not-a-texture.md). |
| `canvas.ruling.style` | `String` | `lines`, `grid` or `dots`. |
| `canvas.ruling.spacing` | `f32` | In page units. 8 to 256 accepted by the server. |
| `stroke.id` | `String` | `"stroke-{n}"`. Unique within a snapshot; not stable across edits on iPad. |
| `stroke.color` | `String` | Honoured by SVG, JSON and the PNG rasterizer. |
| `stroke.width` | `f32` | 0.5–80 accepted by the server. |
| `point.pressure` | `f32` | Real on iPad; `0.5` on desktop and as the web fallback. |
| `point.t` | `u128` | Epoch milliseconds from the web app, per-stroke offset from the iPad. Nothing downstream depends on the meaning. |
| `stroke.startedAt` | `u64?` | When the stroke began, in unix milliseconds on the drawing device's clock (`PKStrokePath.creationDate` on the iPad). Optional at every version; it is what places a stroke next to a spoken sentence, because stroke ids are renumbered on erase. |
| `narration` | `Narration?` | Absent unless someone spoke over the sheet. Requires version 4. See [ADR-0008](../../decisions/0008-narration-is-transcribed-on-the-device.md). |
| `narration.language` | `String` | BCP 47 tag, `nl` for now. Up to 16 characters. |
| `narration.engine` | `String?` | Which model produced the text. Up to 128 characters. |
| `narration.segments[]` | `{ start: u64, end: u64, text: String }` | Spoken stretches on the same clock as `startedAt`. `start <= end`, text up to 2000 characters, up to 4096 segments. The audio itself never crosses the wire. |

### `ExportJson` — what actually lands on disk
A superset of the snapshot, adding:

- `updatedAt` — unix milliseconds at write time. A page and its `latest.*` mirror
  share one stamp, so they can be compared.
- `files` — the *logical* relative paths to this copy's own siblings, so a consumer
  reading only the JSON still learns where they are. A narrated sheet adds
  `files.timeline` and `files.steps`.

Pretty-printed, not minified.

### Ruling in the written files
The rules are drawn **under** the strokes in both the SVG and the PNG, so the page
the agent reads is the page that was drawn on. The ink is fixed in the exporter at
`rule-hair` (`#C9C4BB`) and is never taken from the payload, so nothing on the
network can put arbitrary marks into that file. The first rule falls one space in
from the edge, on both writers, so the iPad and the export agree.

A snapshot with no `ruling` exports byte-for-byte as it always did.

### The narrated package
A sheet with narration also gets a markdown timeline and one crop per step, beside
the three files under the same stem:

```text
drawings/latest.timeline.md            drawings/pages/<pageId>/page.timeline.md
drawings/latest.steps/001.png …        drawings/pages/<pageId>/page.steps/001.png …
```

`src/timeline.rs` turns the sheet into **steps**: one per spoken segment, holding
the strokes begun while that segment was the most recent thing said, plus a
leading step for ink laid down before the first word. A segment that added no
ink folds into the step before it, so every step has ink to show and every
sentence is kept. Above `MAX_STEPS` (200) neighbouring steps are merged evenly.

A **crop** is the window around a step's new ink (stroke widths and halo
included, 24 page units of paper around, clamped to the page), scaled so its long
side is at most 512 px. It is drawn in this order: paper, ruling, a **halo** under
the strokes added in the step (their own path at radius + 6 page units, in
`#D6E4F0`, `stamp-review` lightened), every stroke drawn up to that step in its own
colour and width, then the step's strokes on top. Nothing about a stroke changes;
the halo sits under all ink. Strokes from later steps are not there yet, because
the crop is the sheet as it stood then. The scale is applied to the geometry
before rasterising, never by resampling, so a crop is a handful of exact colours
and is written as an **8-bit palette PNG** (`png` crate, `ColorType::Indexed`);
one is typically 1–20 KB.

The **timeline** names the sheet, counts steps and segments, explains the halo
once, then per step gives the time span (minutes:seconds from the first stroke or
word), quotes every sentence, and says how many strokes were added, where on the
page (`top-left`, `centre`, …) and the window in page units. Crop paths are
relative to the markdown file, so the same text serves `latest.*` and the page copy.

**Staleness.** A sheet written without narration removes `<stem>.timeline.md` and
`<stem>.steps/` under its stem, so the agent can never read yesterday's words
against today's sketch. Crops are rendered into `<stem>.steps.tmp/` and swapped
into place after the three main files; leftovers of an interrupted write are
cleared at the start of the next.

### Where it lands
```text
drawings/latest.{json,svg,png}                 # the most recently written page
drawings/latest.timeline.md, latest.steps/     # only when that page was narrated
drawings/pages/<pageId>/page.{json,svg,png}    # that page's own copy
drawings/pages/<pageId>/page.timeline.md, page.steps/
drawings/pages/index.json                      # every page, newest first
```

`latest.*` keeps its meaning from [ADR-0001](../../decisions/0001-drawings-latest-as-the-agent-contract.md):
it is a byte-identical mirror of whichever page was written last, so consumers that
know nothing about pages are unaffected. The `/goghmode` skill is written to
`~/.claude/skills/` only when the user runs `install-skill`, so installed copies can
never be updated in step with this app — which is what makes the mirror mandatory
rather than convenient.

`index.json` is rebuilt by scanning `pages/*/page.json` after every write, never
maintained incrementally, so it cannot drift and needs no repair path.

## Props / API
```rust
export::write_artifacts(snapshot, directory, stem, link_prefix, updated_at) -> Result<ExportedFiles>
export::snapshot_to_svg(snapshot: &DrawingSnapshot) -> String
export::snapshot_to_rgba(snapshot: &DrawingSnapshot) -> RgbaImage
export::step_window(snapshot, step) -> Option<Window>
export::render_step_crop(snapshot, step, drawn_so_far, window) -> RgbaImage
export::write_palette_png(image, path) -> Result<()>
timeline::build_steps(snapshot, max_steps) -> Vec<Step>
timeline::markdown(snapshot, steps, windows, stem) -> String
pages::write_page(snapshot: &DrawingSnapshot, drawings_dir: &Path) -> Result<ExportedFiles>
pages::list_pages(drawings_dir: &Path) -> Vec<PageEntry>
pages::load_page(drawings_dir: &Path, page_id: &str) -> Result<(DrawingSnapshot, u128)>
pages::page_id_is_safe(page_id: &str) -> bool
```

`write_artifacts` is the only writer; `write_page` and `promote_page` are the two
ways to call it. Uploads go through `write_page`, which writes the page copy, mirrors
it to `latest.*`, then rebuilds the index. `snapshot_to_rgba` is reused by the desktop
**Copy image** action.

## Implementation

### Atomic write
All three files are written as `latest.json.tmp`, `latest.svg.tmp`,
`latest.png.tmp`, then renamed into place. This is the mechanism behind the bridge
invariant *a failed transfer must not corrupt the last good drawing*. A test asserts
no `.tmp` residue is left behind.

### SVG
`<svg>` sized to `ceil(canvas)`, a white background rectangle, then one
`<path d="M … L …">` per stroke — or a `<circle>` for a single-point stroke, so a
dot is not lost. Numbers are trimmed of trailing zeros; all attribute values are
escaped. Points outside the canvas are filtered out of the SVG but **kept in the
JSON**, so a resize does not destroy data.

### PNG
Rasterized by hand: a white `RgbaImage`, a Bresenham walk along each segment,
stamping a filled disc of radius `width / 2` at each pixel. Ink is fixed at
`Rgba([17, 24, 39, 255])`.

Two known divergences, both listed in **Known failure modes** in
[ARCHITECTURE.md](../../ARCHITECTURE.md):

- The PNG ignores `stroke.color`, so iPad colour choices survive only in the SVG and
  JSON.
- The SVG background is always `#ffffff`, so the web app's cream paper is dropped.

## Design tokens
Ink `#111827` / `rgb(17,24,39)`. Background `#ffffff`. Nothing else is fixed —
stroke width and colour travel in the data.

## Tech used
- `serde` derive for the schema; the same structs serialize on the desktop and
  deserialize from untrusted uploads.
- `image` 0.25 with the `png` feature only — encoding is all that is needed, so no
  decoders are compiled in.
- No SVG library; the string is built directly.

## Data
Owns no data. It takes a snapshot in and writes three files out. Nothing reads them
back into the application.

## Data access / permissions
Filesystem only. The directory is chosen by the Mac at startup — see **Where output
lands** in [ARCHITECTURE.md](../../ARCHITECTURE.md).

## Client state
None.

## States

| State | Behaviour |
| --- | --- |
| Default | Three files written, previous contents replaced. |
| Empty drawing | Still valid output: a white page in all three formats. |
| Out-of-bounds points | Filtered from the SVG, retained in the JSON. |
| Write failure | `Err` propagates; the temporary files are the only casualties, `latest.*` stays as it was. |

## Estimate
Shipped. Only remaining work is listed.

| Scope | Estimate |
| --- | --- |
| Schema, SVG, PNG, atomic write | shipped |
| Schema v2 with page identity (Phase 1) | not estimated — see [PLANNING.md](../../PLANNING.md) |
| History alongside `latest.*` (Phase 1) | not estimated |
| **Total** | — |

## Tasks
- [x] Honour `stroke.color` in the PNG rasterizer.
- [x] Decide whether `canvas.background` should be honoured or removed from the
      schema. **Kept, still ignored.** Two shipped clients send it, and removing a
      field from a schema three clients write is a break for the sake of tidiness.
      Ruling is what the sheet's appearance is carried by now.
- [x] Draw ruling into the PNG and the SVG, under the strokes.
- [x] Narration: `startedAt`, `narration`, the timeline and the step crops.
- [ ] Deletion and renaming of pages on the host. The iPad deletes its own copy;
      the host still only ever writes.

## Open questions
- ~~Should ruling exist at all, given the design refused grids?~~ **Answered: yes,
  as a per-sheet writing aid, drawn into the export.** See
  [ADR-0007](../../decisions/0007-ruling-is-a-writing-aid-not-a-texture.md).
- ~~When `schemaVersion` goes to 2, does the Mac accept version 1 uploads from older
  clients, or refuse them?~~ **Answered: it accepts both.** The server takes `{1, 2}`
  and files version 1 uploads under a reserved `legacy` page, so a client that
  predates pages gains history without knowing they exist. Refusing version 1 would
  have bricked every installed companion build, and the three client implementations
  still cannot be upgraded simultaneously.
- When can version 1 be retired? Not before the shipped TestFlight build is known to
  be updated.
