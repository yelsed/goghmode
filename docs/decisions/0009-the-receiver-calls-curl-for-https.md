# ADR-0009 · The hub receiver calls `curl` for HTTPS

- **Status:** Accepted
- **Date:** 2026-11-07

## Context
The receiver fetches from the sketch hub at `https://paperclip.procrast.nl:8441`, which only
speaks TLS. ADR-0004 keeps HTTP frameworks, async runtimes and TLS out of the binary: GoghMode
installs with `cargo install --path .` and ships as one self-contained executable.

## Decision
The receiver runs the `curl` command for its two requests (fetch the inbox, confirm it), with
`--fail-with-body` and `--max-time`, and parses the JSON it prints. `curl` is part of macOS and of
every Arch install.

## Consequences
- No new crate, no TLS stack to keep current; certificates are checked by the system's `curl`
  against the system trust store, which already trusts the hub's Let's Encrypt certificate.
- A machine without `curl` cannot receive; the receiver says so in its first log line.
- One process per request, every few seconds: negligible for one person's machines.

## Alternatives considered
- `ureq` with `rustls`: a self-contained HTTPS client, but it adds a TLS stack and its compile
  time, which is exactly what ADR-0004 turned down.
- Plain HTTP on the tailnet: the hub serves TLS only, and the tailnet name answers with a
  certificate anyway.
