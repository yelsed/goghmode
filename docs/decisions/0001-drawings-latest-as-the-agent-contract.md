# ADR-0001 · `drawings/latest.*` is the agent contract

- **Status:** Accepted
- **Date:** 2026-06-12 _(recorded retroactively 2026-07-25)_

## Context
Several devices can produce a drawing: the Mac window, a phone browser, and an iPad
with an Apple Pencil. Something has to tell the AI agent where the drawing is.

The obvious alternatives all leak the device into the prompt — per-device
directories, timestamped filenames, an index the agent has to read first, or a
command that returns a path. Each one means the person prompting has to know, or
ask, which device drew the page. That is exactly the friction the product exists to
remove: photograph, AirDrop, find the file, hand it over.

## Decision
The Mac always writes the most recent drawing to exactly three paths, overwriting
whatever was there:

```text
drawings/latest.json
drawings/latest.svg
drawings/latest.png
```

Three formats because consumers differ: PNG for image-capable tools, SVG for vector
inspection, JSON for structured stroke data. The `/goghmode` skill and both prompt
strings hardcode these paths, checking `./drawings/` and the app-bundle location
`~/Pictures/GoghMode/drawings/`.

Everything else follows from this: the Mac owns the output directory, every writer
goes through `export::write_snapshot`, and a failed transfer must not corrupt the
last good drawing — hence validation before writing and an atomic `.tmp` + rename.

## Consequences
- Prompting is trivial and device-independent: "look at the latest sketch" always
  resolves.
- Consumers need no discovery logic — no listing, no globbing, no sorting.
- **There is exactly one page.** Drawing again destroys the previous one. This is the
  single biggest limitation of the product today and the reason multi-page support
  is the highest-value deferred item.
- When history arrives it has to be added *alongside* `latest.*`, not instead of it,
  or every consumer breaks. That constraint is baked in.
- Two locations rather than one — terminal launch and app-bundle launch differ — so
  the skill and both prompts must keep checking both.

## Alternatives considered
- **Dated filenames from day one** — solves history, breaks the simple prompt, and
  forces every consumer to sort. Rejected for now; likely to arrive as an addition.
- **A read endpoint on the local server** — makes the Mac the queryable source of
  truth, but adds an API surface to a bridge deliberately kept write-only.
- **Per-device directories** — makes the agent's job harder to save the Mac a
  rename. Rejected outright.
