# ADR-0011 · The hub is a destination beside paired hosts, trusted through the tailnet

- **Status:** Accepted
- **Date:** 2026-11-07

## Context
A paired host proves itself with a key from pairing, and every request to it is signed both ways
(ADR-0006). The sketch hub on paperclip has no pairing: it asks Tailscale which device a request
comes from, and the owner allows the iPad once on the dev-portal page. The iPad reaches only the
hub's port on paperclip (the access policy), over HTTPS with a public certificate.

## Decision
The companion gets a second kind of destination, the hub, next to the paired hosts. Requests to it
are plain JSON over HTTPS, unsigned: the tailnet says who the iPad is, and the certificate says who
the hub is. The hub's answer is per machine, and the companion shows it per machine.

## Consequences
- No pairing step for the hub; a refusal names what to do (allow the iPad on the portal page).
- The hub path cannot reuse the signed client; it gets its own small client for its four routes.
- Anyone on a device the owner allowed can send through the hub, which is the same trust the
  tailnet already gives.

## Alternatives considered
- Pairing the iPad with the hub like a host: a second identity system on top of the tailnet,
  which the design rules out.
- Sending to every paired host directly instead: needs the same Wi-Fi and one pairing per machine,
  the problem the hub exists to remove.
