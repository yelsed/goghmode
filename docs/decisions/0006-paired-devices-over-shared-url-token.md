# ADR-0006 · Paired devices with per-device secrets, not one shared URL token

- **Status:** Accepted
- **Date:** 2026-07-26

## Context
[ADR-0002](0002-token-in-path-lan-pairing.md) made the URL path itself the credential: one random
token, shared by every device, persisted so home-screen shortcuts survive restarts. It was the right
call for a single machine serving a single tablet, and it is explicit about what it accepts —
anything on the local network that knows the path can post a drawing.

Two things break that reasoning.

**Several hosts.** The companion should reach more than one machine: a Mac and an Arch/Omarchy
Linux box. A host is currently identified only by the address it happens to answer on, so a saved
URL that stops matching reality cannot be told apart from a different machine now holding that
address. A drawing lands somewhere the user did not intend, and it looks like a success.

**The asset is sharper than it looked.** `latest.*` is what the coding agent reads. Write access to
it is not "someone can deface my sketch" — it is an input channel into an agent with a filesystem
behind it. That reframes the accepted risk in ADR-0002 from tolerable to worth paying for.

The constraints from ADR-0002 that still hold: no accounts, no cloud, no tunnel, no port forwarding,
and the browser companion is a single HTML file with no build step and deliberately no
credential-handling code.

## Decision
For the **native companion**, replace the shared URL token with paired devices:

- A host generates a random `hostId` at first launch and keeps it forever, at `~/.goghmode/host-id`
  mode `0600`. Identity is deliberately independent of address, port, and hostname, so a machine
  that moves is still recognisably itself.
- Pairing is a single-use secret with a roughly 120-second lifetime, shown as a QR code, and **a
  person taps approve on the host** before a device is admitted.
- **The per-device secret is derived, never transmitted:**
  `deviceSecret = HMAC-SHA256(pairingSecret, "goghmode-device-v1" ‖ deviceId)`, computed
  independently on both sides and recorded on the host in `~/.goghmode/devices.json` mode `0600`.
  The pairing request and response are themselves signed under the pairing secret, so an attacker
  who reads or rewrites every packet of the exchange learns nothing and cannot even raise the
  approval prompt.
- **The first successful pairing disables the legacy unauthenticated route**, with a notice and a
  toggle to re-enable it. An authenticated route beside an anonymous one accepting writes to the
  same directory is not an improvement.
- Every upload carries `HMAC-SHA256(deviceSecret, deviceId ‖ timestamp ‖ nonce ‖ hostId ‖
  SHA-256(body))`. The host checks the device is known and unrevoked, the timestamp is within ±120
  seconds, the timestamp is strictly greater than the last one accepted from that device, and the
  signature matches — **all before the body is parsed** — then answers a single generic `401` for
  any failure. Persisting the last accepted timestamp is what makes replay protection survive a
  restart, which an in-memory set of seen nonces does not.
- Every response carries `HMAC-SHA256(deviceSecret, "response" ‖ nonce ‖ statusCode)`, so the
  companion can prove it reached the host it paired with before it reports success.

One primitive, HMAC-SHA256, with a pre-shared key. No public-key infrastructure, no certificates, no
certificate store per device, no invented cryptography.

For the **browser companion**, ADR-0002 stands unchanged. `mobile/index.html` keeps the secret-URL
scheme and is labelled the lower-trust surface.

Full reasoning, threat model, protocol, and phases:
[companion-multi-host-plan.md](../companion-multi-host-plan.md).

## Consequences
- One device can be revoked without disturbing the others, and without breaking every saved
  shortcut. Today rotating the token revokes everyone.
- A stale address cannot silently reach the wrong machine. The identity does not match and the
  companion says so, distinctly from "unreachable".
- An address or port change stops being destructive: identity is stable, so the profile updates
  instead of the trust being lost.
- Sniffing the network never yields a working credential, because the secret does not travel during
  pairing either.
- The browser companion stops working on a host as soon as that host pairs a device, unless the
  toggle is used. That is the intended trade and it is announced at the moment it happens.
- **Drawing contents are still cleartext on the wire.** This does not fix confidentiality and does
  not pretend to. Transport-layer security with a certificate pinned at pairing time is the upgrade
  path, deliberately not taken.
- Two credential systems coexist for a while, and the desktop has to be honest about which surfaces
  are authenticated.
- Pairing costs one approval tap per device. That tap is also the security property, so it is not a
  cost to optimise away.
- Two dependencies enter a crate that has kept them few: `hmac` and `sha2`.
- Setting up a device now requires the host to be in front of you. That was already effectively true
  — the URL had to be read off its screen.

## Alternatives considered
- **Per-device tokens with no signing** — fixes revocation and wrong-host detection, leaves forgery
  and replay open to anyone who can read one request off the network. Half the benefit for most of
  the work.
- **Public-key host identity** — genuinely stronger, and the right answer if pairing ever has to
  happen over an untrusted channel. Here it happens by pointing a camera at a screen in the same
  room, which is already out-of-band and already authenticated by physical presence. It buys little
  and costs key generation, storage, rotation, and verification in every client. The protocol is
  shaped so it can be swapped in later: `hostId` becomes a key fingerprint, the message
  authentication code becomes a signature, and nothing else changes.
- **Transport-layer security with a self-signed certificate** — adds confidentiality, which nothing
  else here does. It also gives every browser on every device a certificate warning, and the browser
  companion cannot pin anything. Recorded as the upgrade path, not the first step.
- **A relay that fans out to several hosts** — the only design that literally delivers "one URL, two
  hosts", and it puts handwriting on a third machine that must be run, updated, and defended.
  Rejected as a violation of the local-first constraint.
- **Leaving ADR-0002 alone and saving several URLs in the companion** — the cheapest option, and it
  makes the wrong-host problem worse rather than better: more saved addresses, same inability to
  tell whether any of them still points at the machine the user means.
