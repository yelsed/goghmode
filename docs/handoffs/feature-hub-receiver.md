# Handoff: feature/hub-receiver

**What this branch is for:** `goghmode receive --hub <url>`, the receiving end of the dev-portal
sketch hub (phase 7, `docs/phases/phase-7-sketch-hub-receiver.md`). Based on
`feature/narrated-sheets` (PR #14), because narrated sheets travel through the hub from day one.

**State:** built, pull request #15 to `feature/narrated-sheets` open for review; `goghmode-receive.service` runs on paperclip. `src/hub_receiver.rs` applies inbox items through the host's own functions and
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

**Second review, fixed:** a pin of a page this machine cannot read is confirmed and logged; curl
runs with `--disable` so a `~/.curlrc` cannot change its output; the timeout covers connecting and
a stalled transfer, not the total, so a large backlog can finish; the README adds
`loginctl enable-linger` for a server; the write-failure test skips when run as root.

**Second review, not fixed, and why:**
- *A hub sheet that waited in the queue can overwrite a newer direct save of the same page.* The
  receiver cannot tell which is newer: a snapshot carries no time of its own. Once the iPad sends
  through the hub only (plan 3) there is one route and the order is the hub's.
- *An inbox item without a numeric version is logged every round.* Only a hub bug makes one; the
  hub writes every version itself.
- Minors left: the test hub's request reader could reuse `mobile_server::read_http_request`; a
  promote reads its page twice; the one-second floor is applied in two places.

**Third review, fixed:** a pin of a page whose stored file cannot be read is kept (as a paired host
keeps it) and only the promote is skipped; after a failed confirm the next round confirms the
applied items without writing them again; curl ignores proxy variables; the transfer timeout is a
stall (no byte for the timeout), so no hidden speed floor; README and ADR-0009 describe the flags.

**Third review, not fixed, and why:**
- *The hub keeps a re-sent page at its old place in the queue, so it can land before a promote
  that was sent after the first version.* That is the hub's ordering rule (dev-portal
  `docs/sketch-hub-api.md`), chosen so a pin never comes before its sheet. A fix belongs in the
  hub, for example a new sequence when a newer version follows a stamp. Follow-up for dev-portal.
- *A large backlog is held in memory at once and every sheet in it re-renders `latest.*`.* The
  hub has no paging yet, and its queue limit (200) bounds it. Paging in the hub plus writing
  `latest.*` once per round is the fix when a machine is off long enough to matter.
