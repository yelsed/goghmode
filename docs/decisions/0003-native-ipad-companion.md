# ADR-0003 · A native iPad app in addition to the web page

- **Status:** Accepted
- **Date:** 2026-07-25

## Context

The mobile web page proved the loop: draw on a device, tap send, the Mac writes
the files. But it is a poor writing surface on an iPad. Safari gives no palm
rejection worth the name, no eraser, no pencil tools, and `pointerdown`/`move`
strokes lag behind the Pencil in a way that is obvious the moment you write a
sentence rather than draw a box.

Every one of those is something PencilKit already solves and none of them is
something a canvas element will ever solve.

## Decision

Ship a native SwiftUI iPad app (`ipad-companion/`) that draws with
`PKCanvasView` + `PKToolPicker`, converts the `PKDrawing` to the existing
`DrawingSnapshot`, and posts it to the same `/<token>/save` endpoint the web page
uses. It debounces uploads 600 ms after the last stroke change and shows one
status badge that doubles as a retry button.

The web page stays. It is the zero-install path for a phone, an Android device,
or someone else's iPad.

## Consequences

- Writing on the iPad feels like writing: pencil, eraser, lasso, palm rejection,
  and tool state persisted by PencilKit.
- No new server surface. The native app is one more client of a protocol that
  already existed, so validation, the token, and the file contract are untouched.
- The Mac stays the owner of the drawings directory. "Make the Mac and iPad
  equal" is the wrong frame — the Mac is the bridge — see
  [`later.md`](../../later.md) item 2.
- Cost: a second UI to keep in step, and installation means Xcode and a signing
  identity rather than a URL. There is no App Store distribution.
- Two independent visual languages now exist. Divergence is not cosmetic: the
  toolbar originally used a translucent material over white, right above a white
  canvas, and people tried to draw on it and concluded their pen was broken. The
  toolbar now uses the same dark chrome as the Mac (18, 24, 34).
- Uploading the whole drawing after every stroke is fine for short pages and will
  not be once pages are long — [`later.md`](../../later.md) item 3.

## Alternatives considered

- **Keep only the web page** — cheapest, but leaves the main use case (writing
  notes with a Pencil) permanently mediocre.
- **Replace the web page with the native app** — drops the zero-install path for
  every non-iPad device to save maintaining one framework-free HTML file.
- **A cross-platform framework (Flutter, React Native)** — the entire reason for
  going native is one Apple framework. Wrapping it adds a layer whose only job
  would be to get out of the way.
