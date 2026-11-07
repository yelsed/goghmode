# ADR-0010 · A sheet from the hub is written exactly as a host writes it

- **Status:** Accepted
- **Date:** 2026-11-07

## Context
The sketch hub relays the same snapshots and stamps the iPad sends to a paired host (`/v2/save`,
`/v2/pin`, `/v2/promote`). A machine can therefore receive a page both ways. If the two paths
wrote differently, the skills would see different sheets depending on the route, and pins would
mean different things.

## Decision
The receiver applies hub items with the host's own functions: `validate_snapshot`, then
`pages::write_page` for a sheet, `pages::set_pin` for a pin and `pages::promote_page` for a
promote, in the order the hub sent them. A pin on a machine behaves exactly as it does with direct
pairing: `latest.*` follows the pinned page.

An item that can never succeed (it fails validation, names an unknown kind, or promotes a page this
machine does not have) is confirmed and logged, so one bad item cannot block the queue. A write
error confirms nothing from that item on, so it comes again, in order, on the next fetch.

## Consequences
- One code path for what ends up on disk, whatever route a sheet took.
- A pin set on the iPad reaches every ticked machine, as the owner meant it.
- A rejected item disappears from the queue; only the receiver's log says why.

## Alternatives considered
- A receiver-specific writer: two paths that drift apart.
- Leaving rejected items queued: one malformed sheet would stop every later delivery.
