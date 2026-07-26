# Mobile Server API

> Source: `src/mobile_server.rs` · Used on: [mobile-web-canvas](../pages/mobile-web-canvas.md), [ipad-companion](../pages/ipad-companion.md) · Status: done

## Purpose
The local HTTP bridge. It serves the embedded web sketchpad to any device on the
Wi-Fi and accepts drawing uploads from the web app and the iPad companion. It is the
only network surface in the project, so it is also the only trust boundary.

## Anatomy
- Blocking `std::net::TcpListener` on `0.0.0.0:8787`. **On any bind failure it falls
  back to port 0**, an ephemeral port — see **Known failure modes** in
  [ARCHITECTURE.md](../../ARCHITECTURE.md).
- One thread. Connections are handled sequentially with `Connection: close`.
- The listener is non-blocking; the accept loop polls at 25 ms and checks an
  `AtomicBool`. `Drop` sets the flag, self-connects to unblock `accept`, and joins
  the thread.
- The four web assets are `include_bytes!`-compiled into the binary, so there is
  nothing to deploy and no static directory to configure.

## Props / API

```rust
MobileServer::start(drawings_dir: PathBuf) -> Result<MobileServer>
server.url() -> &str   // http://{lan_ip}:{port}/{token}/
```

The displayed IP comes from `preferred_lan_ip()`: bind a UDP socket, `connect()` to
`8.8.8.8:80` without sending a packet, read back the local address; fall back to
`127.0.0.1`. The listener itself always binds `0.0.0.0`.

### Routes
All paths are relative to `route_prefix = "/{token}/"`. Query strings are stripped
before matching.

| Method | Path | Response |
| --- | --- | --- |
| GET / HEAD | `{prefix}` or `{prefix}index.html` | `mobile/index.html`, `text/html` |
| GET / HEAD | `{prefix}manifest.webmanifest` | `application/manifest+json` |
| GET / HEAD | `{prefix}service-worker.js` | `text/javascript` |
| GET / HEAD | `{prefix}icon.svg` | `image/svg+xml` |
| GET / HEAD | `{prefix}capabilities` | `{"schemaVersions":[1,2],"features":["pages"]}`, `application/json` |
| GET / HEAD | `/{token}` (no trailing slash) | `308` redirect to `{prefix}` |
| POST | `{prefix}save` | `200 {"ok":true}` · `400` with a reason · `500` on write failure · **`403` once a device has been paired**, unless the legacy toggle is back on |
| POST | anything else | `405` |
| Any other verb | any | `405` |
| GET | unknown path | `404` |

### Paired-device routes

These sit **outside** the token prefix: a paired device authenticates by signature,
so the path secret has no part to play.

| Method | Path | Response |
| --- | --- | --- |
| GET / HEAD | `/v2/hello` | `{"v","schemaVersions","features","time"}`. A **signed** request additionally gets `hostId`, `name` and `platform` — an unauthenticated one never does, because a stable identifier handed to any scanner is a tracking value. |
| POST | `/v2/pair` | `200` with `{"v","hostId","name","platform"}` and an `X-GoghMode-Pair-Mac` header · `403`, identical for denied, expired, reused, unsigned and wrong |
| POST | `/v2/save` | `200 {"ok":true}` · `400` with a reason · `401`, identical for every authentication failure · `500`. Every answer carries `X-GoghMode-Host-Mac`. |

Request headers on `/v2/save` (and optionally `/v2/hello`):

| Header | Contents |
| --- | --- |
| `X-GoghMode-Device` | `deviceId`, `[A-Za-z0-9_-]{1,64}` |
| `X-GoghMode-Timestamp` | Milliseconds since the Unix epoch |
| `X-GoghMode-Nonce` | 16 random bytes, hex |
| `X-GoghMode-Mac` | `HMAC-SHA256(deviceSecret, deviceId ‖ timestamp ‖ nonce ‖ hostId ‖ SHA-256(body))` |

Fields inside a signed message are **length-prefixed**, not separated by a
delimiter: a device name is arbitrary user text and could otherwise be crafted so
that one field list reads as another.

`time` in `/v2/hello` is public on purpose. A host whose clock is wrong rejects
every upload, and because authentication failures are deliberately opaque that
would otherwise be undiagnosable — the companion compares clocks and says so.

Every response sets `Cache-Control: no-store`.

## Implementation

### Reading the body
`read_http_request` reads until `\r\n\r\n`, parses a case-insensitive
`Content-Length`, then keeps reading until `header_end + 4 + content_length` bytes
are buffered, slicing the body to exactly the declared length and tolerating a
client that sends extra. `read_more` retries on `Interrupted` and turns
`WouldBlock`, EOF, and other errors into human-readable reasons.

Before any of it, `handle_connection` **clears the non-blocking flag the accepted
socket inherited from the listener** and sets a 10-second read timeout. On macOS,
without that, every read past the first returns `WouldBlock`, so any upload spanning
more than one TCP segment failed with a bare 400. That was commit `1877248`;
`tests/mobile_server.rs` now posts a 4000-point drawing in 16 KiB chunks with
deliberate delays between them to keep the fix honest.

### Validation — the trust boundary
`validate_snapshot` runs before anything is written and returns the **first named**
failure, which becomes the 400 body. A bare "invalid" is useless to someone holding
an iPad.

| Limit | Value |
| --- | --- |
| Body size | 4 MiB, checked on headers and on the declared `Content-Length` |
| `schemaVersion` | `1` or `2` |
| `page` | required at version 2, absent at version 1 |
| `page.id` | `[A-Za-z0-9_-]`, 1–64 characters |
| `page.title` | ≤ 200 characters |
| Canvas width / height | finite, `1.0 ..= 4096.0` |
| `canvas.background` | ≤ 64 characters |
| Stroke count | ≤ 4096 |
| `stroke.id` | ≤ 128 characters |
| `stroke.color` | ≤ 64 characters |
| `stroke.width` | finite, `0.5 ..= 80.0` |
| Total points | ≤ 200 000 |
| Every point | finite, and inside the canvas rectangle |

Every rejection also logs `goghmode: rejected upload: {reason}` to stderr.

`page.id` is the sharpest edge here: it becomes a **directory name**, so it is
checked against `[A-Za-z0-9_-]{1,64}` before anything joins it to a path. A test
posts `../escape`, `a/b`, `""`, `/etc/passwd` and a 65-character id, and asserts each
is refused and that nothing appears outside the drawings directory.

Accepting `{1, 2}` rather than bumping to `2` is deliberate: refusing version 1 would
break every already-installed companion, and the three clients cannot be updated
together. Version 1 uploads carry no page and are filed under a reserved `legacy`
page.

`{prefix}capabilities` exists so a companion can ask what a Mac accepts instead of
inferring it from a rejection. A Mac from before pages has no such route and answers
404, which is itself the answer: the iPad drops the page field, sends version 1, and
says so in the UI.

On success it calls `crate::pages::write_page` with the drawings directory captured
at server start — the Mac owning the output directory is what makes that possible.
See [export-contract](export-contract.md).

## Design tokens
Not applicable.

## Tech used
No HTTP framework, no async runtime, no TLS — the reasoning is in
[ADR-0004](../../decisions/0004-no-http-framework.md). `serde_json` parses the body;
`include_bytes!` supplies the assets.

## Data
- **Accepts:** one `DrawingSnapshot` per POST.
- **Writes:** through `export::write_snapshot` only.
- **Reads:** nothing. There is no endpoint that returns a drawing, which is why
  Mac-side page browsing is deferred in [PLANNING.md](../../PLANNING.md).

## Data access / permissions
The token is the route prefix: 16 random bytes hex-encoded from `/dev/urandom`
(with a time-and-PID fallback), persisted at `~/.goghmode/mobile-token` so
home-screen shortcuts keep working across restarts. A stored value is only reused if
it is at least 32 characters and all ASCII hex.

There is no header authentication, no cookie, no CSRF token, no origin check, and no
TLS. Anything on the LAN that knows the path can POST. Accepted risk, reasoned
through in [ADR-0002](../../decisions/0002-token-in-path-lan-pairing.md).

## Client state
The server holds the drawings directory, the token, the bound address, and the
shutdown flag. Nothing per-connection and nothing per-device — which is exactly what
incremental uploads would need.

## States

| State | Behaviour |
| --- | --- |
| Running | Serves assets, accepts saves, the desktop toolbar shows the URL. |
| Bind failed on 8787 | Silently serves on an ephemeral port; previously-copied URLs break with no signal. |
| Start failed entirely | Non-fatal — the desktop app runs with "Mobile server unavailable". |
| Body too large | `400` before reading the whole body. |
| Invalid snapshot | `400` with the named reason; **no file is written**. |
| Write failure | `500`; `latest.*` is untouched thanks to the atomic write. |
| App closing | `Drop` flips the flag, self-connects to unblock `accept`, joins the thread. |

## Estimate
Shipped. Only remaining work is listed.

| Scope | Estimate |
| --- | --- |
| Server, routes, assets, validation | shipped |
| Multi-packet read loop + regression test | shipped |
| Read endpoint for page browsing (Phase 1/3) | not estimated — see [PLANNING.md](../../PLANNING.md) |
| Per-session state for incremental uploads (Phase 4) | not estimated |
| **Total** | — |

## Tasks
- [ ] Surface the port fallback instead of hiding it — warn, or refuse and fail
      loudly.
- [ ] Decide whether a first upload from an unknown device should need confirmation
      on the Mac.

## Open questions
- Should the server expose a read endpoint at all, or should page history stay
  something only the drawing device browses?
- Should local network access be on by default while the window is open, or opt in
  each session?
