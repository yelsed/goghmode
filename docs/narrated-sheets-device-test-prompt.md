# Narrated sheets: install and test on the Mac and the iPad

Paste everything below the line into Claude Code on the Mac that hosts GoghMode. It installs the
build from pull request 14 there, walks the person through the iPad steps, checks the files the
host writes, and reports on the pull request. Written 20 September 2026.

---

You are on my Mac, the GoghMode host. Install the narrated-sheets update from pull request 14
(branch `feature/narrated-sheets`) and help me test it end to end with my iPad. The repository is
at `~/Projects/goghmode`; if it is not there, ask me for the path. The Mac and the iPad are on the
same Wi-Fi.

Work through these steps in order. Tell me plainly when a step needs my hands.

1. **Check out the branch.** `git fetch`, `git checkout feature/narrated-sheets`, `git pull`. Read
   `docs/superpowers/specs/2026-09-19-narrated-sheet-design.md` and
   `docs/decisions/0008-narration-is-transcribed-on-the-device.md` so you know what the feature
   does and what the files it writes look like.

2. **Build and install the host.** Run `cargo test` and confirm it passes. Then
   `cargo install --path . --force`, `goghmode install-app`, and
   `goghmode install-skill --target claude`. Confirm both
   `~/.claude/skills/goghmode/SKILL.md` and `~/.claude/skills/goghmode-narrated/SKILL.md` exist.
   Quit any running GoghMode window (`pkill -x goghmode` is fine), then reopen GoghMode from
   Spotlight so the new binary is the one serving.

3. **Verify the host advertises narration.** Read the mobile URL from the app's connection chip,
   or build it from `~/.goghmode/mobile-token`, and run
   `curl http://<lan-ip>:8787/<token>/capabilities`. Expect `"schemaVersions":[1,2,3,4]` and
   `"narration"` in the features. If that route answers 403 because a device is paired, use
   `curl http://<lan-ip>:8787/v2/hello` and check `schemaVersions` there instead.

4. **Ask me to do the iPad part**, and wait until I say it is done:
   - Open TestFlight on the iPad and update GoghMode Companion to the newest build (the one
     uploaded today by run 35517659477).
   - Open the app. Check in the Mac's Devices view that this iPad is paired; re-pair by QR if not.
   - Open a new sheet. Tap the microphone button in the top bar, between the stamp and undo.
     First time: allow the microphone, then wait while the model downloads (about 626 MB; the
     button shows the percentage).
   - While recording, in Dutch: say one sentence and draw a group of strokes in the top-left; pause;
     say a second sentence and draw a second group in the centre; pause; say a third sentence and
     draw a third group bottom-right. Then go back into the first group, add one stroke there, and
     say a fourth sentence about it.
   - Tap the button again to stop. Wait until the spinner is gone. Stamp the sheet.

5. **Check what the host wrote.** In `~/Pictures/GoghMode/drawings/`:
   - `latest.json` has `"schemaVersion": 4`, a `narration` block with four segments of Dutch text,
     `language` `nl`, an `engine` naming WhisperKit, and `startedAt` on every stroke.
   - `latest.timeline.md` exists with four steps, each quoting its sentence, each naming a crop and
     a region (`top-left`, `centre`, `bottom-right`, `top-left`).
   - `latest.steps/` holds `001.png` to `004.png`. Report each file's size in KB. Open each crop
     as an image and check: the pale blue halo sits under only the ink added in that step; every
     stroke keeps its own colour and width; the fourth crop shows the first group's earlier ink
     unchanged around the new stroke.
   - `pages/<pageId>/` has the same `page.timeline.md` and `page.steps/`.

6. **Run both skills.** Invoke `/goghmode-narrated` and check that you read the timeline first,
   the crops in order, then `latest.png`, and that what I said matches the ink in each step.
   Then invoke `/goghmode` and confirm it behaves exactly as before: a description of the sheet
   with no mention of the timeline.

7. **Edge cases**, one at a time, each time telling me what to do on the iPad and then checking
   the files:
   - Erase one stroke on the sheet. `latest.json` still has all four segments and the timeline is
     rebuilt without that stroke.
   - Start a recording, say one sentence, and leave the sheet for the register **before** tapping
     stop. The words should still arrive within a minute (the transcription runs on in the
     background). If they do not, reopen the sheet: the unfinished recording is picked up,
     transcribed, and sent.
   - Open a fresh sheet, draw without recording, and stamp it. `latest.timeline.md` and
     `latest.steps/` must be gone, and `latest.json` has no `narration` key.

8. **Report.** Write down what passed and what failed, with exact evidence: file paths, JSON
   excerpts, crop sizes, error text, and what I saw on the iPad. Post it as a comment on pull
   request 14 with `gh pr comment 14`. Do not merge. Do not change code unless a fix is obvious
   and small; if you change anything, commit it on this branch with a clear message and say so in
   the comment.
