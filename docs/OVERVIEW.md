# Product Overview

> **Audience:** everyone — product, design, engineering, stakeholders.
> Plain-language tour of what the app does, feature by feature. Read the top line
> of each section in a meeting; expand the details for depth. This doc
> **summarizes and links** — it never re-explains the specs.

## What is GoghMode?

GoghMode is a sketchpad for people who work with terminal AI tools. You draw a
diagram, a wireframe, an arrow between two boxes — and the drawing lands on disk
as three files a coding agent can read. Then you type `/goghmode` in Claude Code
and the agent describes or acts on what you drew. No screenshot, no upload to a
website, no copy-pasting image data around.

There are three ways to draw: a small macOS window, a web page that same window
serves to a phone or tablet, and a native iPad app that draws with Apple Pencil
through PencilKit. All three write to the same place, so the agent never has to
guess which drawing is current. Everything stays on the local network — no
account, no hosted backend, no public URL.

## Features

🔵 planned · 🟡 in progress · 🟢 shipped

### 🟢 Mac sketchpad
One window: a paper canvas, a brush-width slider, and Save / Undo / Clear.
Releasing the pointer autosaves, so a drawing is never one forgotten click away
from being stale. `Copy image` puts the PNG on the clipboard; `Send to Claude`
and `Print prompt` hand you the text that points an agent at the files.

### 🟢 One drawings directory, three formats
Every save writes `latest.png`, `latest.svg`, and `latest.json` into
`~/Pictures/GoghMode/drawings/` — image for vision-capable tools, vector for
inspection, JSON for structured stroke data. The location does not depend on how
the app was launched, and each file is written to a temporary path and renamed,
so a failed save cannot corrupt the last good drawing. `--drawings-dir`
overrides the location on purpose.

### 🟢 Claude Code skill
`goghmode install-skill --target claude` installs a `/goghmode` skill that tells
the agent which files to read, has it warn you when the drawing is hours old, and
handles the case where a stale project-local `drawings/` directory is lying
around too.

### 🟢 Draw from a phone or tablet (web)
While the Mac app is open it serves a drawing page on the local network, at a URL
containing a long random token. Draw with finger, mouse, or stylus, tap
`Send to Mac`, and the Mac writes the same three files. Export buttons keep a
copy on the device. Installable to the home screen as a PWA.

### 🟢 iPad companion with Apple Pencil
A native SwiftUI app drawing through PencilKit — real pencil tools, palm
rejection, an eraser — which uploads 600 ms after each stroke. A status badge
shows Ready / Waiting / Saving / Saved / Offline, and tapping it retries. Paste
the Mac's mobile URL once and it is remembered.

### 🟢 macOS app bundle
`goghmode install-app` writes `~/Applications/GoghMode.app`, so the sketchpad
opens from Spotlight or Raycast instead of only from a terminal.

### 🔵 Multiple pages and a notes overview
Today there is exactly one page and it is overwritten. Being able to keep written
work is the highest-value deferred item — the reasoning and a proposed first
slice are in [`later.md`](../later.md); the contract it must preserve is
[ADR-0001](decisions/0001-latest-files-contract.md).

### 🔵 Pairing without copy-paste
A QR code for the mobile URL, so a changed port or a reinstalled iPad is a
two-second re-pair instead of a debugging session — see [`later.md`](../later.md)
item 4.

## Where to go next
- **How it's built** → [ARCHITECTURE.md](ARCHITECTURE.md)
- **What's next, in what order** → [PLANNING.md](PLANNING.md)
- **Why we chose X** → [decisions/](decisions/README.md)
- **The longer product bet** → [ai-field-notebook-vision.md](ai-field-notebook-vision.md)
