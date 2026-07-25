# GoghMode

Native sketchpad for Claude Code and other terminal AI tools.

Draw in a small macOS app, or draw from a phone on the same Wi-Fi. GoghMode saves the latest sketch as:

- `latest.png` for image-capable tools
- `latest.svg` for vector inspection
- `latest.json` for structured stroke data

The phone view is served by the desktop app itself. There is no hosted backend, no ngrok-style exposure, and no public URL.

## Requirements

- macOS for the desktop app bundle.
- Rust and Cargo to install from this source checkout.
- Claude Code only if you want the `/goghmode` command.
- Phone or iPad on the same Wi-Fi as the Mac for mobile drawing.

## Install

From this project directory:

```bash
cargo install --path .
```

Install the macOS app bundle:

```bash
goghmode install-app
```

This creates:

```text
~/Applications/GoghMode.app
```

Install the Claude Code skill:

```bash
goghmode install-skill --target claude
```

This creates:

```text
~/.claude/skills/goghmode/SKILL.md
```

After that, you can use `/goghmode` in Claude Code.

## Open the app

Use either path:

```bash
goghmode
```

or open **GoghMode** from Spotlight, Raycast, or `~/Applications/GoghMode.app`.

However it is opened, drawings are always saved in the same place:

```text
~/Pictures/GoghMode/drawings/latest.png
~/Pictures/GoghMode/drawings/latest.svg
~/Pictures/GoghMode/drawings/latest.json
```

To save somewhere else, pass the directory explicitly:

```bash
goghmode --drawings-dir ./drawings
```

Earlier versions saved to `drawings/` relative to the terminal's working directory, so a terminal
launch and a Spotlight launch wrote to different places, and each terminal directory kept its own
separate history. If a stale `drawings/` directory is still lying around from that, it is safe to
delete.

## Pages

Every save also keeps its own copy, so nothing is overwritten out of existence:

```text
~/Pictures/GoghMode/drawings/pages/<pageId>/page.png
~/Pictures/GoghMode/drawings/pages/<pageId>/page.svg
~/Pictures/GoghMode/drawings/pages/<pageId>/page.json
~/Pictures/GoghMode/drawings/pages/index.json
```

`latest.*` keeps its meaning — the page written most recently — so the
`/goghmode` skill and anything else reading those three files is unaffected.
The iPad names its own pages, the Mac canvas writes `mac-scratch`, and the
browser companion gets one page per browser.

Pages are kept forever. Nothing deletes them on a timer.

## Desktop controls

- Draw directly on the paper canvas.
- Release the mouse or trackpad to autosave.
- `Save` writes the latest files immediately.
- `Undo` removes the last stroke and saves.
- `Clear` clears the canvas and saves.
- `Copy image` copies the PNG to the system clipboard.
- `Send to Claude` copies the prompt text for Claude or another AI terminal.
- `Print prompt` writes the prompt to the terminal.
- `Copy mobile URL` copies the local phone URL.
- `Canvas` / `Pages` switches between drawing and browsing saved pages.
- In `Pages`, click a page to point `drawings/latest.*` at it; `Reveal drawings
  folder` opens the folder in Finder.

## Use with Claude Code

Fastest path:

1. Draw something.
2. Type this in Claude Code:

   ```text
   /goghmode
   ```

The skill reads the latest files and tells Claude what to inspect.

If the skill is not installed, use:

```bash
goghmode prompt --target claude
```

Then paste the output into Claude Code.

## Use from a phone or iPad

1. Open GoghMode on the Mac.
2. Keep the Mac and phone on the same Wi-Fi.
3. Click `Copy mobile URL` in the desktop app.
4. Send that URL to the phone, or type the `Mobile: http://...` URL into the phone browser.
5. Draw on the phone.
6. Tap `Send to Mac`.
7. In Claude Code, type:

   ```text
   /goghmode
   ```

The mobile URL includes a persistent random secret path. It only works while GoghMode is open on the Mac. The mobile save endpoint accepts drawing snapshots only and writes only to the configured drawings directory.

Mobile buttons:

- `Send to Mac` writes the drawing into the Mac drawings directory.
- `Share PNG` opens the phone share sheet when the browser supports it.
- `Export PNG`, `Export SVG`, and `Export JSON` keep files on the phone.
- `Undo`, `Clear`, and `Brush` work locally in the phone browser.

## Update after code changes

From this project directory:

```bash
cargo install --path .
goghmode install-app
goghmode install-skill --target claude
```

Quit any already-running GoghMode window, then reopen it.

## Troubleshooting

- **GoghMode does not appear in Spotlight:** run `goghmode install-app`, wait for Spotlight indexing, then search again.
- **The app opens and closes immediately:** run `goghmode install-app` again so the bundled launcher and signed helper binary are refreshed.
- **No drawing files exist yet:** draw one stroke or click `Save`.
- **`/goghmode` is unavailable:** run `goghmode install-skill --target claude`, then restart Claude Code.
- **The phone cannot open the mobile URL:** keep GoghMode open, keep both devices on the same Wi-Fi, and use the exact URL from `Copy mobile URL`.
- **`Send to Mac` fails:** keep the desktop app open and reload the phone page from the current mobile URL.
- **Image paste does not work in an AI interface:** use `/goghmode`, `Send to Claude`, or `goghmode prompt --target claude`.
- **The drawing is too thick or too thin:** adjust `Brush` before drawing.
