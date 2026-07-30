# ADR-0001 · The agent interface is three files called `latest.*`

- **Status:** Accepted
- **Date:** 2026-06-11

## Context

The point of GoghMode is that a coding agent can look at what you drew. Some way
of handing a drawing to Claude Code had to exist, and the candidates were an API
the agent calls, a clipboard hand-off, or files on disk.

Agents differ in what they can consume: some read images, some do not; some are
good at structured data. And whatever the mechanism, the user should not have to
explain where the drawing is every time — the prompt has to be short enough to
type without thinking.

## Decision

Every save overwrites exactly three files in one directory:

```text
~/Pictures/GoghMode/drawings/latest.png    image, for vision-capable tools
~/Pictures/GoghMode/drawings/latest.svg    vector, for inspection
~/Pictures/GoghMode/drawings/latest.json   strokes, for structured reading
```

The directory does not depend on how the app was launched, and every writer —
Mac canvas, phone browser, iPad — goes through `export::write_snapshot`, which
writes to a temporary file and renames it into place. `latest` always means "the
drawing you touched most recently".

## Consequences

- The `/goghmode` skill and the prompt strings are three fixed paths. Nothing has
  to be discovered, configured, or passed as an argument.
- Any tool that can read a file is a supported client, with no integration work.
- A half-finished write cannot destroy the previous drawing: the rename is
  atomic.
- **Only one drawing exists at a time.** Yesterday's page is gone. This is the
  single biggest limitation of the product today, and it is the thing multi-page
  support has to fix without breaking the contract — see
  [`later.md`](../../later.md) item 1.
- The agent can read a stale drawing without noticing, so the skill is
  responsible for checking the modification time and saying so.
- Earlier versions resolved the directory relative to the working directory,
  which produced a different history per terminal tab. Fixed by pinning it to
  `~/Pictures/GoghMode/`; `--drawings-dir` still overrides it deliberately.

## Alternatives considered

- **An HTTP endpoint the agent calls** — needs the agent to speak a protocol,
  handle a port, and be running at the same time. Files work with every tool,
  including ones that do not exist yet.
- **Clipboard only** — invisible to a terminal agent and destroyed by the next
  copy.
- **Timestamped filenames from day one** — keeps history, but then the prompt has
  to name a file, and "the latest one" becomes a directory listing the agent has
  to reason about. History is worth adding *beside* `latest.*`, not instead of it.
