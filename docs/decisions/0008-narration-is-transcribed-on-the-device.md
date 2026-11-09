# ADR-0008 · Narration is transcribed on the drawing device, and only text crosses the wire

- **Status:** Accepted
- **Date:** 2026-09-19

## Context
A whiteboard explanation is easy to follow while it is being drawn and unreadable
afterwards, because everything ends up on top of everything else. GoghMode already
knows when each stroke was made, so a sheet has a timeline. Recording what was said
while drawing, and tying each sentence to the ink added while it was said, lets an
agent read the sheet in the order it was drawn instead of as a finished tangle.

Three questions had to be settled: where speech becomes text, what crosses the
network, and what the agent is handed.

The user speaks Dutch, wants everything to stay local, and wants the option of a
model that learns their own voice later.

## Decision
- **Speech becomes text on the iPad**, with WhisperKit (Whisper on the Neural
  Engine). Apple's on-device recognisers were considered: `SFSpeechRecognizer`
  has a practical one-minute limit per request, and the iOS 26 `SpeechAnalyzer`
  does Dutch on-device for free but cannot be personalised. Whisper can be
  fine-tuned on the user's own recordings and loaded back through the same
  framework, which is the path the user asked to keep open.
- **The audio stays on the device.** It is kept beside the sheet as 16 kHz mono
  WAV because it is the training data for that later model; it is never uploaded.
  The wire carries only text segments with start and end times.
- **One clock, two tracks.** Every stroke gains `startedAt` and every segment has
  `start` and `end`, all in unix milliseconds from the iPad's own clock. The host
  only sorts. Stroke ids cannot anchor anything because they are renumbered when
  an earlier stroke is erased.
- **Schema version 4 is asked for only by a sheet that carries narration**, the
  same rule ruling follows for version 3, so a sheet nobody spoke over changes
  nothing and an old host still receives the strokes.
- **The agent is handed a markdown timeline and one small crop per step**, not
  full-page frames. Every image an agent reads costs tokens; a crop shows only the
  window where ink was added, scaled to at most 512 px, as an 8-bit palette PNG of
  a few kilobytes. The ink added in a step sits on a pale blue halo drawn under
  all ink, so no stroke is recoloured or thickened and earlier ink crossing the
  halo stays legible.
- **A second skill, not a bigger one.** `/goghmode` is unchanged; `/goghmode-narrated`
  reads the timeline. The app must keep working for people who never speak over
  a sheet, and an installed skill can be older than the app.

## Consequences
- The host gains a `timeline` module, a crop renderer and a palette encoder, and
  `write_artifacts` writes or removes two more outputs. A plain sheet written
  after a narrated one removes the timeline and crops, so words can never outlive
  the ink they were spoken over.
- The iPad gains a 600 MB model download on first use, a microphone permission,
  and audio files that grow with use (about 2 MB per minute). Deleting a sheet
  deletes its audio.
- Transcription happens when recording stops, not live, so there is a wait after
  stopping that grows with the recording. Live captions are a later refinement.
- Segments are Whisper's phrase-sized units, so a stroke is placed against the
  sentence being spoken, not the word. Word timings are available from the same
  API if that ever matters.

## Alternatives considered
- **Transcribe on the host with whisper.cpp** — keeps the iPad thin, but puts a
  heavy native dependency into the Rust binary, sends audio over the LAN, and
  makes the host's speed the bottleneck.
- **A cloud transcription API** — best Dutch, but audio leaves the network, which
  the product promises it never does.
- **Full-page frames per step** — the most legible, and the most expensive: a
  page costs roughly 1 900 tokens per frame, and a talk has dozens of steps.
- **Extending `/goghmode`** — one skill fewer, but the plain sheet is still the
  ordinary case and the skill text is installed separately from the app.
