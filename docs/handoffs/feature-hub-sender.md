# Handoff: feature/hub-sender

**What this branch is for:** the iPad companion sends through the dev-portal sketch hub (phase 8,
`docs/phases/phase-8-sketch-hub-sender.md`): the hub as a destination, machines ticked as a
standing selection, live sending, and the state per machine. Stacked on `feature/hub-receiver`
(PR #15).

**State:** built, waiting for CI. New files in `ipad-companion/GoghModeCompanion/`:
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
