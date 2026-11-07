# Phase 7: the sketch hub receiver

Part of the sketch hub: one centre on paperclip (the dev-portal) that the iPad sends to from
anywhere on the tailnet, and that delivers each sketch to the machines the owner ticks. Design:
dev-portal `docs/superpowers/specs/2026-11-07-sketch-hub-design.md`. Contract: dev-portal
`docs/sketch-hub-api.md`. Plan: `docs/superpowers/plans/2026-11-07-hub-receiver.md`.

This phase builds the receiving end, `goghmode receive --hub <url>`: a background process on every
machine (paperclip, omarchy, the MacBook) that fetches what the hub queued for that machine and
writes it into `~/Pictures/GoghMode/drawings/`, so the `goghmode` and `goghmode-narrated` skills
read it unchanged. The hub (plan 1) is built and runs on paperclip on port 8441. The iPad side is
plan 3.

Task: goghmode `TASK-1`. The pull request goes to `feature/narrated-sheets` (stacked on PR #14),
and moves to `master` with it. After the build, goghmode is installed on paperclip and
`goghmode-receive.service` runs there (owner's word, 7 November 2026).

## Checklist (each line is proven by a test)

- [ ] A sheet from the hub is written as `latest.*`, exactly as `/v2/save` would write it, into a
      drawings directory that is created when missing.
- [ ] A pin that arrives in the same fetch as the sheet it names points `latest.*` at that page.
- [ ] A sheet that can never be written (unsupported schema, unsafe page id, unknown kind) is
      confirmed and logged, so it cannot block the queue.
- [ ] A write failure stops before later items, confirms nothing from that item on, and they come
      again in order.
- [ ] Fetching and confirming go through `curl`; a 403 or 503 from the hub becomes one log line
      carrying the hub's own reason, and the receiver tries again after the interval.
- [ ] `goghmode receive` needs `--hub`; the interval (5 s) and the timeout (20 s) are flags.
- [ ] Service files for systemd (paperclip, omarchy) and launchd (MacBook) run it at login.

## Not in this phase

- The iPad app sending to the hub (plan 3).
- Showing a rejected sheet back on the iPad. The hub reports it as delivered; the receiver's log
  says why it was set aside.
- Installing or starting the service on omarchy or the MacBook: the owner decides that per machine.
