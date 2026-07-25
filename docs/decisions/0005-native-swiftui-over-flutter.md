# ADR-0005 · Native SwiftUI and PencilKit for the iPad companion

- **Status:** Accepted
- **Date:** 2026-06-13 _(recorded retroactively 2026-07-25)_

## Context
The browser sketchpad works on every device, but Apple Pencil in Safari is
noticeably worse than in a native app: latency, smoothing, palm rejection, and rapid
lift-and-restart all suffer, and there is no system tool palette. Since the iPad is
meant to be the *primary* writing surface, that gap undermines the whole product.

Two constraints shaped the choice:

- A Flutter project already existed, and it **could not be installed** using the
  available Apple Developer account. Whatever else Flutter offers, a build that
  cannot reach the device is worthless here.
- Any native route requires Xcode or a Mac build service with Apple signing, which
  was not available when the browser companion was built. The browser app therefore
  had to remain the fallback path in the meantime.

## Decision
Build the companion as a native SwiftUI app using Apple frameworks only, with
PencilKit as the drawing surface. Universal iPhone and iPad target, iOS 17 minimum.
Deliver through TestFlight via GitHub Actions rather than cable installs.

Keep the browser sketchpad. It is not a stopgap — it is the zero-install path for
any device without the companion.

## Consequences
- Handwriting feels right: pressure, smoothing, palm rejection, and low latency come
  from Apple's own stack rather than a canvas polyfill.
- The system `PKToolPicker` supplies pen, **eraser**, lasso, colours, and widths for
  effectively no code, and persists the user's tool choice across launches through
  `stateAutosaveName`. The "there is no eraser" complaint was fixed by wiring up the
  picker, not by building an eraser.
- Two client codebases to keep on the same wire schema, in two languages, forever.
- Distribution now depends on Apple: signing certificates, provisioning profiles,
  App Store Connect, and a CI workflow with seven secrets. The setup traps are
  documented in the release section of [ARCHITECTURE.md](../ARCHITECTURE.md).
- Choosing PencilKit meant choosing to convert its strokes rather than its rendering
  — see [ADR-0003](0003-vector-strokes-over-png-upload.md). The exported PNG
  therefore does not look exactly like what the iPad displays.
- Apple-only. There is no Android companion and no plan for one; Android devices use
  the browser app.

## Alternatives considered
- **Flutter** — one codebase for both platforms, but the existing project could not
  be installed with the available Apple account, and its canvas would still not match
  PencilKit for Pencil input. Rejected on the hard constraint, not the preference.
- **Browser app only** — no signing, no store, no second codebase. Rejected because
  Pencil input in Safari is the weak point the native app exists to fix, though the
  browser app is deliberately kept alongside.
- **React Native or a WebView wrapper** — inherits the browser's input quality while
  adding all of the App Store overhead. Worst of both.
