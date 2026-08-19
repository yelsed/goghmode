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

## Copy a sheet to the clipboard

Put a saved drawing on the system clipboard without opening the app:

```bash
goghmode copy
```

Without arguments it copies the sheet your agent reads: the stamped one, or the one drawn on last
if nothing is stamped. It prints which file it copied and how long ago that sheet was saved, so a
fresh sketch is easy to tell apart from yesterday's. To copy some other page, pass its id from
`pages/index.json`:

```bash
goghmode copy --page 9F2C4A1B
```

`--drawings-dir` works here too, for drawings kept somewhere other than the default directory.

### Bind it to a hotkey with Raycast

```bash
goghmode install-raycast
```

This creates:

```text
~/Library/Application Support/GoghMode/raycast/copy-latest-sheet.sh
```

One manual step is left. In Raycast, open Extensions, Script Commands, add that folder, then give
`Copy latest GoghMode sheet` a hotkey. The newest sheet is then one keypress away from any field
you can paste an image into.

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

## The desktop window

The desktop app stopped being a drawing surface. It owns the drawings directory, runs the local
server your devices post to, and shows you what it holds. Drawing happens on the iPad or the phone.

- **Pages** is the home view: every sheet the host has received, newest first, with the stamped one
  marked. Click a sheet to point `drawings/latest.*` at it.
- **Devices** is where pairing lives: it shows a QR code to scan, asks you to approve each new
  device, and lists what is paired, when each was last seen, and why any attempt was refused.
- The connection chip carries the mobile URL for a phone or anything else that cannot pair.
- `Reveal drawings folder` opens the directory in Finder.

To put a sheet on the clipboard, use `goghmode copy` rather than the window; see
[Copy a sheet to the clipboard](#copy-a-sheet-to-the-clipboard).

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
6. Tap `Send to desktop`.
7. In Claude Code, type:

   ```text
   /goghmode
   ```

The mobile URL includes a persistent random secret path. It only works while GoghMode is open on the Mac. The mobile save endpoint accepts drawing snapshots only and writes only to the configured drawings directory.

Mobile buttons:

- `Send to desktop` writes the drawing into the host's drawings directory.
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
- **No drawing files exist yet:** draw one stroke on a paired device, or send a sheet from its register.
- **`/goghmode` is unavailable:** run `goghmode install-skill --target claude`, then restart Claude Code.
- **The phone cannot open the mobile URL:** keep GoghMode open, keep both devices on the same Wi-Fi, and use the exact URL from the connection chip.
- **The iPad says the desktop is an older version:** run `cargo install --path . --force` and
  `goghmode install-app`, then reopen the app. Pages, stamping and ruling each need a host new
  enough to understand them, and the iPad says which one is missing.
- **`Send to desktop` fails:** keep the desktop app open and reload the phone page from the current mobile URL.
- **Image paste does not work in an AI interface:** use `/goghmode`, or `goghmode copy` to put the
  sheet on the clipboard, or `goghmode prompt --target claude` for the prompt text.
- **The drawing is too thick or too thin:** pick a thinner pen in the iPad tool picker, or adjust
  `Brush` in the phone browser.
