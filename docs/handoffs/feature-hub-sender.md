# Handoff: feature/hub-sender

**What this branch is for:** the iPad companion sends through the dev-portal sketch hub (phase 8,
`docs/phases/phase-8-sketch-hub-sender.md`): the hub as a destination, machines ticked as a
standing selection, live sending, and the state per machine. Stacked on `feature/hub-receiver`
(PR #15).

**State:** built; CI green (run 42467756023: all tests pass, build on TestFlight). Not yet tried on the iPad. New files in `ipad-companion/GoghModeCompanion/`:
`SketchHubClient.swift` (the four hub routes), `MachineSelection.swift` (the ticked set, kept in
`UserDefaults`), `MachineControl.swift` (the chip, the machine list and its rows). The hub is a
`HostCredential.sketchHub` host; `UploadController` sends sheets and stamps to it for the ticked
machines and keeps `hubDeliveries` per page. Tests: `GoghModeCompanionTests/SketchHubTests.swift`.

**How it is tested:** no machine here has Xcode, so the tests run in the `iOS TestFlight` workflow
on this branch (`gh workflow run ios-testflight.yml --ref feature/hub-sender`). The first run
failed on the missing types, as intended (RED).

**Differs from the design:** a ticked machine the hub stopped listing reads "not fetching from the
hub now" instead of "since 6 Nov": the hub drops such a machine from `/hub/machines`, so the iPad
has no date to show.

**Review (8 November 2026), fixed with tests:** a sheet or pin every machine refused is a
refusal, not a save; a refresh keeps the refusals the deliveries route does not list; ticking a
machine sends the sheet that waited for one; stamp failures are worded for their own
destination; the machine list shows a complaint over a stale list and ignores a cancelled
request; hub state is forgotten with the host; the date formatters are made once.

**Review, not fixed, and why:**
- *A refused hub sheet offers retry, also for a 413.* A 403 is the common refusal, and retrying
  after allowing the iPad is what the owner wants; a sheet over 4 MiB is rare and the hub's
  sentence names the limit.
- *The hub is wired in as checks in `UploadController` rather than one sender per destination.*
  A real cleanup, worth doing when a third destination kind arrives; today it would rewrite the
  paired path this phase promised to leave untouched.
- *The list polls machines and deliveries one after the other.* Two small requests every three
  seconds while the list is open.

**Still to do by hand:** allow `ipad148` on `https://dev.procrast.nl/`, update the companion in
TestFlight, add the hub, tick machines and draw; the receiver on paperclip writes the sheet.
