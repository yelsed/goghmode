# Handoff: feature/hub-receiver

**What this branch is for:** `goghmode receive --hub <url>`, the receiving end of the dev-portal
sketch hub (phase 7, `docs/phases/phase-7-sketch-hub-receiver.md`). Based on
`feature/narrated-sheets` (PR #14), because narrated sheets travel through the hub from day one.

**State:** built. `src/hub_receiver.rs` applies inbox items through the host's own functions and
fetches and confirms over `curl`; `goghmode receive` and the service files in `contrib/receiver/`
are in. 179 tests pass.

**Formatting:** `cargo fmt --check` fails on the base branch itself (the files were formatted with
another rustfmt). This branch keeps that churn out and checks only the files it changes with
`rustfmt --check`.

**Checked so far:** the crate builds on paperclip (no display) on 7 November 2026, so the receiver
can run there.

**Checked by hand on 7 November 2026, on paperclip:** before paperclip was allowed, the receiver
logged the hub's refusal once. After it was allowed on `https://dev.procrast.nl/`, a sheet queued
with curl (`hub-smoke-test`) was written as `latest.*`, and `/hub/deliveries` reported it as
`delivered`.

**Review (7 November 2026), fixed:** an unusable pin page id and an unreadable promote are now
confirmed and logged; one malformed inbox item no longer stops the whole fetch; what was written
is logged before confirming; curl runs without `--fail-with-body` (needs 7.76, older macOS lacks
it) and is always reaped; the systemd user unit no longer names `network-online.target`, which a
user manager cannot order against.

**Review findings not fixed, and why:**
- *Two writers in one drawings directory (host and receiver) share the fixed `.tmp` names in
  `export.rs` and `pages.rs`.* Fixing it means changing the host's own write path, outside this
  branch. It only bites when the iPad saves to a paired host on the same machine at the same
  moment a hub sheet lands; paperclip runs no host. Worth a lock or unique temporary names when
  the iPad sends through the hub only (plan 3).
- *A sheet that passes `validate_snapshot` but always fails `write_page` blocks the queue.* The
  host has the same gate for `/v2/save`, and telling a disk error from a content error needs
  `export.rs` to classify its errors. Until then a stuck item shows in the journal every round.
