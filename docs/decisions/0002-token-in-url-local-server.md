# ADR-0002 · Phone access is a hand-written local server behind a secret URL

- **Status:** Accepted
- **Date:** 2026-06-12

## Context

Drawing with a mouse is bad. The good drawing surface is a phone or a tablet, so
those devices need a way to get a drawing onto the Mac — which means the Mac has
to accept a network write.

That is the only place in this project where something outside the machine can
cause a file to be written, so the constraints were: no public exposure, no
account system to build, no tunnel to a third party, and no dependency that would
turn a small tool into a web stack.

## Decision

While the Mac window is open it runs a small HTTP server, written directly on
`std::net`, bound to `0.0.0.0` on port 8787 (falling back to a random port).

Every route lives under a secret prefix: a 16-byte token from `/dev/urandom`,
hex-encoded, persisted in `~/.goghmode/mobile-token` so a home-screen shortcut
keeps working across restarts. Without the token, everything is `404`.

`POST /<token>/save` is the only mutating route. It takes a `DrawingSnapshot`,
runs it through `validate_snapshot`, and writes only into the configured drawings
directory — no part of the request reaches a path. `GET` serves the four embedded
`mobile/` assets and nothing else.

## Consequences

- Setup is "copy the URL to the phone". No pairing screen, no login, no cloud.
- The attack surface is: one POST route, one JSON shape, on the local network,
  only while the app is open. `validate_snapshot` bounds every field —
  dimensions, stroke and point counts, widths, string lengths, and a 4 MB body
  cap — and it names what was wrong so the phone can show something actionable.
- No framework means no dependency updates and a tiny binary, but also that every
  HTTP detail is ours: multi-packet request bodies, `HEAD`, redirects, and macOS
  inheriting the listener's non-blocking flag onto accepted sockets were all bugs
  we had to fix by hand.
- Anyone on the same Wi-Fi who has the URL can write a drawing. Acceptable for a
  personal tool on a home or office network; it is not an access-control model
  for a shared or hostile network.
- The silent fallback to a random port makes a previously-copied URL stop working
  with no explanation — a real usability trap, tracked in
  [`later.md`](../../later.md) item 4.

## Alternatives considered

- **A web framework (axum, warp, tiny_http)** — more correct HTTP for free, at
  the cost of a dependency tree far larger than the app for four static files and
  one POST.
- **A tunnel (ngrok and similar)** — puts a private sketchpad on the public
  internet and adds an account. Rejected outright.
- **AirDrop or a shared folder** — no autosave, and every drawing becomes a
  manual file operation.
- **Pairing confirmation in the Mac app per session** — more secure, but it turns
  a two-second capture into a two-device ceremony. Revisit if this ever runs on a
  network we do not control.
