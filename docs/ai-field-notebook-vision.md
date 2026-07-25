# AI Field Notebook Vision

## Premise

GoghMode should become an always-ready sketchbook, field notebook, and whiteboard for working with an AI agent.

The current real-world friction is too high: writing in a physical notebook is fast, but sharing it with the Mac requires taking a photo, moving it over AirDrop, finding the file, and then giving it to the agent. The app exists to remove that loop. Capture should feel as immediate as opening a notebook, and sharing should be automatic.

## Product goal

Create a notebook surface that can be opened instantly on the best available device, used naturally with pen or touch, and made available to the desktop AI workflow without manual file transfer.

The core promise:

1. Open the notebook.
2. Write, sketch, explain, or diagram.
3. The Mac receives the latest page automatically.
4. The AI agent can inspect it through stable local files or a connected knowledge base.

## Devices

### iPad

The iPad should be the primary writing device because Apple Pencil gives the best experience for handwriting, sketching, and whiteboarding.

The preferred long-term implementation is a native iPad app using PencilKit. PencilKit handles low-latency Apple Pencil input, pressure, smoothing, palm rejection, and rapid lift/restart behavior better than a browser canvas.

### iPhone

The iPhone should work as a quick-capture fallback when the iPad is not available.

The phone experience does not need to match the iPad for long-form writing. It should support quick sketches, short notes, photo capture, and sending existing images into the same desktop bridge.

### Mac

The Mac remains the bridge to the AI agent. It owns the local output directory and exposes the latest notebook page through stable files.

Current files to preserve:

- `drawings/latest.json`
- `drawings/latest.svg`
- `drawings/latest.png`

These files are already compatible with terminal AI workflows and should remain the first integration contract.

## Capture modes

### Notebook

A persistent page or small page stack for handwritten notes. This is for thoughts, lists, and explanations that should survive beyond a single session.

### Field note

A quick note mode for capturing something immediately. It should open fast, save automatically, and require no naming before use.

### Whiteboard

A larger canvas for diagrams, system sketches, arrows, layouts, and spatial thinking. It should be easy to clear or start a fresh board without losing the previous export.

### Snapshot import

On iPhone especially, the app should accept a photo or image and send it to the same desktop bridge. This keeps the physical notebook workflow possible while removing AirDrop and manual file handling.

## AI sharing contract

The AI agent should not need to know which device created the page. It should read from one stable place.

The desktop app should continue to write the latest capture into `drawings/latest.*`. Later, it can also keep a history of pages, but the latest page contract should stay stable because it makes prompting simple.

Possible agent-facing commands:

- Inspect latest notebook page.
- Use latest sketch as context.
- Save latest page into project notes.
- Attach latest page to a task or design discussion.

## Obsidian integration

Obsidian should be considered a storage and review layer, not the first capture surface.

A later integration can write notebook pages into the existing Obsidian-based LLM wiki vault. A page could become:

- A markdown note with metadata.
- A linked image export.
- A JSON or SVG source file stored beside the note.
- A backlink to the project or conversation where it was used.

This should be added after the basic capture-to-agent loop is reliable.

## Deployment constraint

The app must be deployable under the current Apple tooling constraints.

Flutter is not a good fit here because the existing Flutter project cannot currently be installed using the available Apple Developer enabled account. The PencilKit route should use native SwiftUI and Apple frameworks only.

However, full native iPad development still requires Xcode or a Mac build service with Apple signing. Until that is available, the browser companion remains the fallback path, with the known limitation that Apple Pencil behavior is worse than PencilKit.

## Implementation direction

### First working version

Keep the existing Mac Rust app and local server. Improve the current browser companion enough to keep the project useful while Xcode access is blocked.

Important browser improvements:

- Better handling for rapid pen lift and restart.
- Coalesced and predicted pointer events where Safari allows them.
- Clear warning when the page is running over insecure local HTTP and high-quality pointer APIs are unavailable.
- Optional image upload so a phone photo can still enter the same desktop bridge.

### Native companion version

When Xcode or another Apple signing path is available, add a native iPad and iPhone companion app.

Recommended native flow:

1. User opens GoghMode on the Mac.
2. Mac shows a local URL, QR code, or pairing code.
3. User opens the iPad or iPhone app.
4. The app stores the paired Mac endpoint.
5. User writes in a PencilKit canvas.
6. The app converts PencilKit strokes to the current JSON snapshot shape.
7. The app posts the snapshot to the Mac.
8. The Mac exports `latest.json`, `latest.svg`, and `latest.png`.

Converting PencilKit strokes to the existing JSON format is preferred over sending PencilKit binary data first. It preserves the current export pipeline and keeps the desktop bridge simple.

## Non-goals for now

- Hosted accounts.
- Cloud sync.
- Public tunnels.
- Replacing Obsidian.
- Building a full notes application before the capture-to-agent loop is excellent.

## Open decisions

- Whether the first native app is iPad-only or universal iPad and iPhone from the start.
- Whether pages are stored only as latest capture first, or as a dated page history immediately.
- Whether Obsidian export should be automatic or explicit.
- Whether the whiteboard and notebook are separate modes or one canvas with templates.
- Which Apple signing route is available first: local Xcode, cloud Mac build, TestFlight, or direct device install.
