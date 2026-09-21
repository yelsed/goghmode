# Narrated sheets, first device test on the Mac (20 September 2026)

First end to end run of pull request 14 on the real host and a real iPad. A sheet was recorded,
transcribed on the iPad and written by the host: `schemaVersion` 4, `language` `nl`, engine
`whisperkit/openai_whisper-large-v3-v20240930_626MB`, 12 strokes all carrying `startedAt`, 10
spoken segments folded into 5 steps, crops of 739 B to 2.5 KB. The halo sits under only the ink
added in each step, earlier ink keeps its colour and width, and `/goghmode-narrated` read the
timeline first and then the crops in order. The feature works.

What follows is what the run turned up. Nothing here is fixed: this file is the hand-over to
whoever picks the branch up next.

## 1. Whisper's silence marker is quoted as if it had been said

The first step quoted `***`. WhisperKit writes a run of asterisks for a stretch it heard no words
in, and the host takes it for a sentence: it opens a step of its own, and in markdown `> ***`
renders as a horizontal rule, so the step reads as an empty quote.

Evidence from the run, in `latest.json`:

```json
{ "start": 1789933346910, "end": 1789933347910, "text": "***" }
```

and in `latest.timeline.md`:

```markdown
## Step 1 · 00:00–02:06 · latest.steps/001.png
> ***
Drawn: 4 strokes, centre · window x 235–694, y 202–512.
```

A shape that was tried and then backed out again, kept here as a suggestion rather than a patch:
drop segments that hold no alphanumeric character, after validation and before the sheet is
written, so a version refusal still names the real reason; and write a sheet whose narration is
nothing but such segments as an unnarrated one, so no stale timeline survives. The same filter
belongs on the iPad, in the mapping in `NarrationRecorder`, so the words never leave the device,
with the host keeping its own filter for an older app.

## 2. Recording appears to run while the model is still loading, and a tap throws it away

What the user saw, in their words: while the model was loading, and once it had loaded, it had
already been recording for about thirty seconds. Tapping the button put it back to clear, and the
recording then started fresh. So the first half minute was captured, shown as if it counted, and
then silently discarded by a tap that read as "stop".

Two things to settle:

- What the control means during `preparingModel`. Either it does not present itself as recording
  until the model is ready, or the audio captured while preparing is kept and transcribed when the
  model lands. Showing a running recording and then dropping it is the one option to avoid.
- What a tap does in that state. Right now it resets. If audio was captured, a tap should stop and
  transcribe it like any other recording, not clear it.

## 3. Iterating on a sheet should work from a copy, not reload every slice

The user's idea, to carry into the next round: when a sheet is iterated on, it should be possible
to go on from a copy of the drawing, so that not all slices are loaded again every time. Today
every write rebuilds the whole package, every step crop is rendered again from the full snapshot,
and an agent rereading the sheet opens every crop from the start, so the cost grows with the
length of the talk while only the tail has changed.

Not designed yet. Shapes worth weighing: keeping the crops of steps whose strokes and words are
unchanged instead of rerendering them; a copy of the sheet taken at a point that later steps build
on; or a timeline that names what changed since the previous write so a reader can open only the
new steps.

## 4. Not covered by this run

- The four regions. Everything was drawn in the middle of the page, so the step lines say `centre`
  four times and `left` once. The `top-left` / `centre` / `bottom-right` wording from the test
  script was never exercised.
- The edge cases in `docs/narrated-sheets-device-test-prompt.md` step 7 (erasing a stroke, leaving
  the sheet before stopping, a fresh unnarrated sheet clearing the timeline) were not run, because
  the interface work came first.
