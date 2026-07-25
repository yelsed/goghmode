# ADR-0004 · Hand-written HTTP instead of a framework

- **Status:** Accepted
- **Date:** 2026-06-12 _(recorded retroactively 2026-07-25)_

## Context
The desktop app needs to serve four small static files and accept one kind of POST,
on a local network, while its window is open. The obvious choice is a web framework
— `axum` and `hyper` pull in `tokio`; `tiny_http` is lighter but still a dependency
and still a threading model to reason about alongside the `eframe` event loop.

The distribution constraint matters more than it looks. GoghMode installs with
`cargo install --path .` and then copies a single binary into a macOS app bundle.
Anything that adds compile time, a second runtime, or files that must sit next to
the binary makes that story worse.

## Decision
Write the server directly on `std::net::TcpListener`: parse the request line, read
`Content-Length`, read the body, match a small route table, write the response. One
thread, connections handled sequentially, `Connection: close` on every response.
Embed all four web assets with `include_bytes!`.

No HTTP framework, no async runtime, no TLS, no mDNS or Bonjour crate, no QR crate.

## Consequences
- The whole network layer is one readable file with no async colouring, and it
  composes trivially with the blocking `eframe` event loop — the server is started in
  the app's constructor and stopped by `Drop`.
- The binary is genuinely self-contained: the web app ships inside it, so there is no
  static directory to locate, install, or get out of step with the executable.
- Compile times and the dependency surface stay small.
- **Reading a request body correctly is now the project's problem.** It bit exactly
  once: accepted sockets on macOS inherit the listener's non-blocking flag, so every
  read past the first returned `WouldBlock` and any upload spanning more than one TCP
  segment failed with a bare 400. A framework would have handled this. The fix is
  three lines and a regression test that posts a 4000-point drawing in 16 KiB chunks.
- Sequential handling means one slow client blocks the next. Irrelevant at one or two
  devices; it would not survive real concurrency.
- There is no routing, middleware, or content negotiation to lean on if the surface
  ever grows — a read endpoint for page history would be hand-written too. That is
  the point at which this decision deserves revisiting.
- Changing the embedded web app requires recompiling the binary, and an installed
  progressive web app still serves the cached shell until the service worker cache
  name is bumped.

## Alternatives considered
- **`axum` / `hyper` + `tokio`** — correct HTTP, real routing, and an async runtime
  living beside a blocking GUI event loop for the sake of five routes.
- **`tiny_http`** — the closest call. It would have prevented the multi-packet bug
  for roughly the same amount of code. Worth reconsidering if the server gains read
  endpoints.
- **Serve the web assets from disk** — simpler code, but the app bundle would need to
  carry them and keep them in step with the binary.
