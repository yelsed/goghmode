# ADR-0002 · The pairing token is the URL path

- **Status:** Accepted
- **Date:** 2026-06-12 _(recorded retroactively 2026-07-25)_

## Context
The desktop app serves a drawing surface to phones and tablets on the same Wi-Fi.
Something has to stop a random device on that network from overwriting the drawing
the agent is about to read.

The constraints are unusual and worth stating, because they are what make the
conventional answers wrong:

- There are no accounts, and adding them is an explicit non-goal.
- Plain HTTP over a LAN. TLS would need a certificate no local device trusts, so the
  browser would show a warning and the high-quality pointer APIs would still be
  unavailable.
- The web client must work with no credential-handling code at all — it is a single
  HTML file with no build step.
- The URL has to survive being saved as a home-screen shortcut, so whatever the
  credential is, it must live *in* the URL and must be stable across restarts.

## Decision
Generate 16 random bytes from `/dev/urandom`, hex-encode them, and make that string
the **route prefix**: everything the server serves lives under `/{token}/`. Persist
it at `~/.goghmode/mobile-token` and reuse it, validating that a stored value is at
least 32 characters of ASCII hex.

The web app therefore needs no authentication code whatsoever — it posts to the
relative URL `save`, and whatever secret path served the page is the path it writes
to. The iPad app stores the pasted URL and appends `save`.

There is no `Authorization` header, no cookie, no CSRF token, no origin check, and
no TLS.

## Consequences
- The client side is free. No token plumbing, no header logic, no expiry handling.
- Home-screen shortcuts and stored iPad endpoints keep working across restarts,
  because the token is persisted rather than regenerated.
- **Anything on the LAN that knows the path can POST a drawing.** The secret is in
  the URL, so it is visible in browser history, in a screenshot of the toolbar, and
  over the wire in cleartext. This is the accepted risk for a tool that runs on a
  home or office network only while its window is open.
- Because the credential rides in the URL and the token is stable, a URL that stops
  working is almost always a *port* problem, not an auth problem — the server
  silently falls back to an ephemeral port when 8787 is taken. That failure presents
  as a permanently offline iPad with no useful signal.
- The real defence against a bad payload is not the token but `validate_snapshot`,
  which rejects malformed input before anything is written.

## Alternatives considered
- **A bearer token in a header** — no better against a LAN attacker who can read the
  URL anyway, and it costs credential-handling code in three clients plus a way to
  get the token onto the phone that is not the URL.
- **TLS with a self-signed certificate** — browser warnings on every device, a
  trust-store dance per device, and no gain against someone already on the network.
- **Explicit pairing confirmation on the Mac** — genuinely stronger, and still open
  as a question. It costs a modal on every new device and a device registry the
  server does not currently have.
- **No secret at all, bind to localhost only** — kills the entire feature; the point
  is that another device draws.
