# Product Overview

> **Audience:** everyone — product, design, engineering, stakeholders.
> Plain-language tour of what the app does, feature by feature. Read the top line
> of each section in a meeting; expand the details for depth. This doc
> **summarizes and links** — it never re-explains the specs.

## What is GoghMode?

GoghMode is a sketchpad that puts a hand-drawn page in front of a terminal AI agent
without any file shuffling. You draw on a Mac window, in a phone browser, or on an
iPad with an Apple Pencil, and the drawing lands on the Mac as
`drawings/latest.png`, `latest.svg`, and `latest.json`. In Claude Code you then type
`/goghmode` and the agent looks at what you just drew.

It exists to delete one specific loop. Writing in a paper notebook is fast; getting
that page to an agent is not — photograph it, AirDrop it, find the file, hand it
over. GoghMode makes capture as immediate as opening a notebook and sharing
automatic. Everything runs on your own machine and your own Wi-Fi: no accounts, no
cloud, no public tunnel, no hosted backend.

## Features

Status marker per feature: 🔵 planned · 🟡 in progress · 🟢 shipped.

### 🟢 The desktop bridge (macOS, Linux)
Not a drawing surface. It owns the drawings directory the agent reads, runs the local
server the devices post to, and approves which devices may use it. Two views: the
register of every sheet it holds, and the devices it is paired with. It stopped
drawing in July 2026, because once the iPad is the good surface a second, worse one
competing for the same directory is a liability.
See [desktop-bridge](specs/pages/desktop-bridge.md).

### 🟢 Phone and tablet sketchpad in the browser
The desktop app serves a small drawing web app over the local network — no install,
no app store. Open the URL the Mac shows, draw with a finger or stylus, tap
**Send to desktop**. It installs to the home screen as a progressive web app.
See [mobile-web-canvas](specs/pages/mobile-web-canvas.md).

### 🟢 Native iPad companion (Apple Pencil)
A SwiftUI app built on PencilKit, so pressure, smoothing, palm rejection, and the
standard Apple tool palette — pen, eraser, lasso, colours — behave the way they do
in Apple's own apps. It uploads automatically 600 ms after you stop drawing, keeps
the screen awake while you draw (on by default), and says plainly what to do when
the Mac is not reachable. Pairing remembers the Mac's network name as well as its
address, so a changed Wi-Fi costs a fallback first and at most one scan to
re-pair — never a re-setup.
See [ipad-companion](specs/pages/ipad-companion.md).

### 🟢 One stable output contract
Whichever device drew the page, the Mac writes the same three files. That stability
is the whole point: the agent never has to know where a drawing came from.
See [export-contract](specs/components/export-contract.md) and
[ADR-0001](decisions/0001-drawings-latest-as-the-agent-contract.md).

### 🟢 Claude Code skill and macOS app bundle
`goghmode install-skill` writes a `/goghmode` skill into `~/.claude/skills/`, and
`goghmode install-app` builds `~/Applications/GoghMode.app` so the sketchpad opens
from Spotlight or Raycast like any other Mac app.

### 🟢 iPad delivery through TestFlight
The companion ships via a GitHub Actions workflow that tests on a simulator, signs,
and uploads to App Store Connect. See **Releasing the iPad companion** in
[ARCHITECTURE.md](ARCHITECTURE.md).

### 🟢 A register of sheets
Every page is a sheet in a numbered drawing set, kept on the iPad and mirrored on
the host. Exactly one sheet carries the issue stamp, and that is the one the agent
reads. Sheets can be renamed, stacked into a series, swiped away, and stepped back
through: a sheet keeps its last twenty states, so closing it and opening it again
does not lose the ability to take a stroke back.
See [ipad-companion](specs/pages/ipad-companion.md).

### 🟢 Ruling on the sheet
A sheet can be plain, lined, squared or dotted, chosen per sheet. The rules are
drawn into the exported PNG and SVG under the ink, so the page the agent reads is
the page that was written on. Off by default.
See [ADR-0007](decisions/0007-ruling-is-a-writing-aid-not-a-texture.md).

### 🟢 Paste the latest sheet anywhere
`goghmode copy` puts the stamped sheet on the system clipboard, and
`goghmode install-raycast` gives it a hotkey. One keypress, then paste: into an
Obsidian note, a chat window, anywhere that takes an image. See the README.

### 🟢 Pairing by QR code
The host shows a pairing code, the iPad scans it with the camera, and the person at
the host approves the device. The long-lived key is derived on both sides rather
than sent, so nothing secret crosses the network.
See [ADR-0006](decisions/0006-paired-devices-over-shared-url-token.md).

### 🔵 Photo and snapshot import
Sending an existing photo of a paper page through the same bridge, so the physical
notebook workflow survives without AirDrop.

## Device roles

| Device | Role |
| --- | --- |
| iPad | The primary writing surface. Apple Pencil is the best experience for handwriting, sketching, and whiteboarding. |
| iPhone | Quick-capture fallback — short notes and sketches when the iPad is not around. |
| Mac | The bridge. It owns the output directory, runs the local server, and is what the agent reads from. |

## Non-goals

- No accounts, no cloud sync, no hosted backend.
- No public tunnels or externally reachable URLs — local Wi-Fi only.
- Not a replacement for Obsidian. Writing pages into the Obsidian vault is a later
  storage layer, not the capture surface.
- No full notes application before the capture-to-agent loop is boring and reliable.

## Where to go next
- **How it's built** → [ARCHITECTURE.md](ARCHITECTURE.md)
- **Exact screen detail** → [specs/](specs/README.md)
- **Why we chose X** → [decisions/](decisions/README.md)
- **What's next** → [PLANNING.md](PLANNING.md)
- **The longer-form vision** → [ai-field-notebook-vision.md](ai-field-notebook-vision.md)
