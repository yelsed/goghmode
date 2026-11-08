# Phase 8: the iPad sends through the sketch hub

Part of the sketch hub: one centre on paperclip (the dev-portal) that the iPad sends to from
anywhere on the tailnet, and that delivers each sketch to the machines the owner ticks. Design:
dev-portal `docs/superpowers/specs/2026-11-07-sketch-hub-design.md`. Contract: dev-portal
`docs/sketch-hub-api.md`. The hub (plan 1) runs on paperclip on port 8441; the receiver (plan 2,
phase 7) runs on paperclip as `goghmode-receive.service`.

This phase builds the sending end in the iPad companion: the hub becomes a destination next to the
paired hosts. With the hub chosen, the owner ticks machines once; the ticked set is remembered and
always visible, and every sheet, pin and promote goes to the hub for those machines, live while
drawing, narration included. The iPad shows per machine whether a sheet is queued, refused or
delivered.

Task: goghmode `TASK-2`. Branch `feature/hub-sender`, stacked on `feature/hub-receiver` (PR #15),
so the pull request goes to `feature/hub-receiver` and moves down with it.

## How it fits the app

- The hub address is `https://paperclip.procrast.nl:8441`. That name resolves publicly to
  paperclip's tailnet address (checked 7 November 2026, also for `dev.procrast.nl`), and the
  certificate is valid on that port, so the iPad needs no `*.ts.net` resolution (YEL-280).
- The hub knows the iPad from its tailnet address (ADR-0011); there is no pairing key and no
  signature. The iPad is allowed once on the dev-portal page.
- The body of a sheet is exactly the body a paired host gets on `/v2/save`, so ruling and narration
  (schema version 4) travel unchanged and the receiver writes them as a host would (ADR-0010).

## Decisions in pickup

- The hub is one entry in the host list, next to the paired hosts (owner, 8 November 2026).
  Choosing it shows the machine checkboxes on the drawing screen, and sending goes only to the
  hub, never to a host at the same time.
- Build and test: paperclip has no Xcode, so the Swift tests run in the repository's own
  `iOS TestFlight` workflow on a GitHub macOS runner, started on this branch with
  `gh workflow run ios-testflight.yml --ref feature/hub-sender`, as `feature/narrated-sheets` did.
  A green run also puts the build on TestFlight, which is how the owner tries it on the iPad.

## Checklist (each line is proven by a test)

- [ ] The hub can be added as a destination with its address; it is saved, listed with the paired
      hosts and can be chosen like one.
- [ ] With the hub chosen, the machines come from `GET /hub/machines`; the ticked set is remembered
      across launches, and a ticked machine the hub no longer lists stays visible as unavailable.
- [ ] A sheet goes to `POST /hub/sheets` once, with the ticked machines as `targets` and the same
      snapshot a host gets, about 600 ms after a change, as uploads to a host do.
- [ ] With nothing ticked nothing is sent, and the app says to tick a machine.
- [ ] Pin and promote go to `POST /hub/stamps` for the ticked machines.
- [ ] The state per machine shows queued, refused with the hub's reason, or delivered, from the
      send answer and `GET /hub/deliveries`; never one combined "sent".
- [ ] A 403 shows the hub's own sentence (allow the iPad on the dev-portal page); no connection
      shows as offline and retries, as with a host.
- [ ] Direct pairing with a host works exactly as before.

## Not in this phase

- Replacing direct pairing.
- Showing why a receiver set a sheet aside; the hub reports it as delivered, the receiver's log
  says why.
- A receiver for Windows.
