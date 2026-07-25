# Export Contract

> Source: `src/drawing.rs`, `src/export.rs` · Used on: [desktop-canvas](../pages/desktop-canvas.md), [mobile-web-canvas](../pages/mobile-web-canvas.md), [ipad-companion](../pages/ipad-companion.md) · Status: done

## Purpose
The single definition of what a drawing *is* and what the Mac writes to disk. Every
drawing surface serializes to this shape, and every write goes through one function,
so the agent reading `drawings/latest.*` never has to care which device drew the
page. See [ADR-0001](../../decisions/0001-drawings-latest-as-the-agent-contract.md).

## Anatomy

### `DrawingSnapshot` — the wire format
```jsonc
{
  "schemaVersion": 2,
  "page": { "id": "9F2C4A1B", "title": "Server sketch" },
  "canvas": { "width": 1100.0, "height": 699.5, "background": "#ffffff" },
  "strokes": [
    {
      "id": "stroke-1",
      "color": "#111827",
      "width": 4.0,
      "points": [ { "x": 12.5, "y": 40.0, "pressure": 0.5, "t": 1785312000000 } ]
    }
  ]
}
```

| Field | Type | Notes |
| --- | --- | --- |
| `schemaVersion` | `u8` | `1` or `2`. Current writers send `2`; `1` is still accepted so installed clients keep working. `#[serde(rename)]` on the Rust side; the Swift and JavaScript clients spell it out. |
| `page.id` | `String` | Required at version 2, absent at version 1. Minted by the client that created the page and immutable after. Becomes a directory name, so the server restricts it to `[A-Za-z0-9_-]{1,64}`. |
| `page.title` | `String?` | Optional label for the overview. Up to 200 characters. |
| `canvas.width` / `.height` | `f32` | The live drawing surface size, not a fixed page size. |
| `canvas.background` | `String` | Sent by clients, **ignored on export** — the SVG hardcodes `#ffffff`. |
| `stroke.id` | `String` | `"stroke-{n}"`. Unique within a snapshot; not stable across edits on iPad. |
| `stroke.color` | `String` | Honoured by SVG, JSON and the PNG rasterizer. |
| `stroke.width` | `f32` | 0.5–80 accepted by the server. |
| `point.pressure` | `f32` | Real on iPad; `0.5` on desktop and as the web fallback. |
| `point.t` | `u128` | Epoch milliseconds from the web app, per-stroke offset from the iPad. Nothing downstream depends on the meaning. |

### `ExportJson` — what actually lands on disk
A superset of the snapshot, adding:

- `updatedAt` — unix milliseconds at write time. A page and its `latest.*` mirror
  share one stamp, so they can be compared.
- `files` — the *logical* relative paths to this copy's own siblings, so a consumer
  reading only the JSON still learns where they are.

Pretty-printed, not minified.

### Where it lands
```text
drawings/latest.{json,svg,png}                 # the most recently written page
drawings/pages/<pageId>/page.{json,svg,png}    # that page's own copy
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
export::write_snapshot(snapshot: &DrawingSnapshot, drawings_dir: &Path) -> Result<ExportedFiles>
export::snapshot_to_svg(snapshot: &DrawingSnapshot) -> String
export::snapshot_to_rgba(snapshot: &DrawingSnapshot) -> RgbaImage
pages::write_page(snapshot: &DrawingSnapshot, drawings_dir: &Path) -> Result<ExportedFiles>
pages::list_pages(drawings_dir: &Path) -> Vec<PageEntry>
pages::load_page_snapshot(drawings_dir: &Path, page_id: &str) -> Result<DrawingSnapshot>
pages::page_id_is_safe(page_id: &str) -> bool
```

`write_artifacts` is the only writer; `write_snapshot` and `write_page` are the two
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
- [ ] Decide whether `canvas.background` should be honoured or removed from the
      schema, since nothing uses it today.
- [ ] Deletion and renaming of pages. The current design only ever writes.

## Open questions
- ~~When `schemaVersion` goes to 2, does the Mac accept version 1 uploads from older
  clients, or refuse them?~~ **Answered: it accepts both.** The server takes `{1, 2}`
  and files version 1 uploads under a reserved `legacy` page, so a client that
  predates pages gains history without knowing they exist. Refusing version 1 would
  have bricked every installed companion build, and the three client implementations
  still cannot be upgraded simultaneously.
- When can version 1 be retired? Not before the shipped TestFlight build is known to
  be updated.
