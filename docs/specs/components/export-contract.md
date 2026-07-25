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
  "schemaVersion": 1,
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
| `schemaVersion` | `u8` | Must be exactly `1`. `#[serde(rename)]` on the Rust side; the Swift and JavaScript clients spell it out. |
| `canvas.width` / `.height` | `f32` | The live drawing surface size, not a fixed page size. |
| `canvas.background` | `String` | Sent by clients, **ignored on export** — the SVG hardcodes `#ffffff`. |
| `stroke.id` | `String` | `"stroke-{n}"`. Unique within a snapshot; not stable across edits on iPad. |
| `stroke.color` | `String` | Honoured by SVG and JSON, **ignored by the PNG rasterizer**. |
| `stroke.width` | `f32` | 0.5–80 accepted by the server. |
| `point.pressure` | `f32` | Real on iPad; `0.5` on desktop and as the web fallback. |
| `point.t` | `u128` | Epoch milliseconds from the web app, per-stroke offset from the iPad. Nothing downstream depends on the meaning. |

### `ExportJson` — what actually lands in `latest.json`
A superset of the snapshot, adding:

- `updatedAt` — unix milliseconds at write time.
- `files` — the *logical* relative paths `drawings/latest.{json,svg,png}`, hardcoded
  so a consumer reading only the JSON still learns where its siblings are.

Pretty-printed, not minified.

## Props / API
```rust
export::write_snapshot(snapshot: &DrawingSnapshot, drawings_dir: &Path) -> Result<()>
export::snapshot_to_svg(snapshot: &DrawingSnapshot) -> String
export::snapshot_to_rgba(snapshot: &DrawingSnapshot) -> RgbaImage
```

`write_snapshot` is the only writer. `snapshot_to_rgba` is reused by the desktop
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
- [ ] Honour `stroke.color` in the PNG rasterizer.
- [ ] Decide whether `canvas.background` should be honoured or removed from the
      schema, since nothing uses it today.

## Open questions
- When `schemaVersion` goes to 2, does the Mac accept version 1 uploads from older
  clients, or refuse them? Three client implementations cannot be upgraded
  simultaneously.
