# ADR-0003 · Clients upload vector strokes, not rendered images

- **Status:** Accepted
- **Date:** 2026-06-13 _(recorded retroactively 2026-07-25)_

## Context
When a phone or iPad finishes a drawing, it can send the Mac either the strokes that
produced it or a rendered image of it. Sending an image is easier on the client:
every drawing surface can already produce a PNG, and PencilKit in particular has a
first-class `drawing.image(from:scale:)`. Sending strokes means writing a converter
per client and keeping three implementations of one schema in step.

The Mac already had a stroke model, an SVG writer, and a hand-rolled rasterizer for
the desktop canvas, all driven by `DrawingSnapshot`.

## Decision
Every client converts its own strokes into `DrawingSnapshot` and posts that JSON.
The Mac renders. No client ever uploads a rendered image.

On iPad this means walking each `PKStroke`'s path, taking the mean of
`max(width, height)` as the stroke width, extracting the colour as hex, and rounding
coordinates to hundredths and pressure to thousandths before clamping.

## Consequences
- One export pipeline. The Mac produces all three output formats from one input, and
  a fix to the SVG writer benefits every surface at once.
- The JSON output is genuinely structured — an agent can read stroke order, timing,
  and pressure, not just pixels.
- Drawings stay re-exportable at any size later, because vectors were kept.
- **The schema is now a three-language contract.** Changing `DrawingSnapshot` means
  changing Rust, Swift, and JavaScript together, plus `validate_snapshot`. The Swift
  tests assert the JSON key shape against the Rust struct specifically to catch
  drift.
- Rounding is load-bearing: full `Double` precision costs roughly 250 bytes per point
  on the wire and the Mac stores `f32` anyway. It also introduced a subtle ordering
  bug — rounding *after* clamping can push an edge point outside the canvas and earn
  a 400 — now locked down by a test.
- Payloads grow with page length, and the iPad re-sends the whole drawing 600 ms
  after every stroke. Not a problem at current page sizes; it becomes one as soon as
  multi-page notebooks exist.
- PencilKit's own rendering is not reproduced. The Mac's rasterizer draws flat discs
  along each segment, so the exported PNG does not match what the iPad displays —
  and it ignores stroke colour entirely.

## Alternatives considered
- **Upload a PNG** — trivial per client, but the Mac would be a file copier, the JSON
  output would be meaningless, and nothing could be re-rendered at another size.
  Still worth adding as an *extra* path for clients that can only produce raster
  images, such as photo import.
- **Upload PencilKit's native binary drawing data** — highest fidelity, but it
  requires Apple frameworks to decode, which a Rust binary does not have, and it
  would not work for the web client at all.
- **Both, negotiated per client** — two code paths on the Mac to serve one feature.
  Rejected until something actually needs the raster path.
