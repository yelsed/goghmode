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
