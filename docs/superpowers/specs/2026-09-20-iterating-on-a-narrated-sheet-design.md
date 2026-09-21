# Iterating on a narrated sheet without reloading every step (design, 20 September 2026)

## Context

A narrated sheet is rebuilt in full on every write. `write_artifacts` renders every step crop
from the whole snapshot each time the iPad posts (every 600 ms while drawing, and after each
transcription), and an agent that rereads the sheet opens every crop from step one. Both costs
grow with the length of the talk while only the tail changed. The user's framing, from the first
device test: it should be possible to go on from a copy of the drawing, so that not all slices are
loaded again every time.

Three shapes were weighed. They are not exclusive; the first two share their bookkeeping.

## Option A. Keep the crops whose ink and words did not change

Host-side, invisible to the person. Each step gets a key: a hash over its window, the ruling, the
geometry, colour and width of every stroke drawn up to and including that step that reaches into
the window, and the text of its sentences. `<stem>.steps/` gains a small manifest
(`steps.json`: index, key, file). On a write, a step whose key matches the manifest keeps its
file; only steps with a new key are rendered. Erasing a stroke or a sentence folding differently
changes the keys from that point on, so correctness does not depend on appends only.

- Cuts render cost to the changed tail. A thirty minute talk stops costing thirty minutes of
  crops per stroke.
- Changes nothing the agent reads. The timeline and the crop set are as today.
- Small: one hash, one manifest, one `if` in `write_step_crops`. Rust only.

## Option B. Continue on a copy of the sheet

A gesture on the iPad, in the register: **Continue on a copy**. It makes a new sheet holding the
same strokes, with their original `startedAt`, and no narration; the new sheet's title block says
which sheet it continues. Nothing changes on the wire or the host: the copied strokes all predate
the first new word, so the timeline's leading step shows the inherited ink in one crop as "drawn
before", and every further step is new work. The earlier words stay with the earlier sheet.

- Matches the user's words. A long-running sheet gets a clean start whenever they choose, and the
  agent reading the copy sees the base drawing plus only the new steps.
- Costs the link between old words and old ink on the new sheet; the old sheet keeps it.
- iPad work: a `PageStore.copy(_:)`, a menu item, a `continues` field on the page for the title
  block. Optional `page.continues` on the wire later, so the timeline can name the base sheet.

## Option C. A timeline that names what changed since the previous write

Using the same manifest as A, the host writes one line under the header: which steps are new or
changed since the last write, and which are unchanged. An agent that read the sheet earlier in
the same conversation opens only the named steps.

- Cuts what a rereading agent opens, which is the token cost the user feels most.
- Needs A's manifest and nothing else; without a memory of the previous read it is just a line.

## Decision (21 September 2026)

A and C, on the host, chosen by the user and built: `steps.json` beside the crops, unchanged
crops hard-linked from the previous write, and a line under the timeline header naming the
steps that changed. B stays open as the person-facing gesture.

## Recommendation

**A and C together, on the host, first.** One manifest serves both, no gesture is needed, and the
two costs that grow with the talk both stop growing. **B on top**, as the person-facing gesture,
because it is what the user described and it also answers a different need: a fresh start on a
sheet that has become a whole talk. It is independent of A and C and can land in its own slice.

## Not chosen

- Rendering crops lazily on read. There is no read path on the host, by design (ADR-0001), and the
  agent reads plain files.
- Incremental uploads of strokes. Planned separately (PLANNING.md, phase 4) and orthogonal: the
  cost here is rendering and reading, not the upload.
