# Mobile Companion Roadmap

## Ultimate flow

1. User starts `goghmode` on the development machine.
2. User opens a GoghMode companion app on iPhone, iPad, or another mobile device.
3. The desktop bridge and mobile app pair on the local network with a short code or QR code.
4. User draws on the mobile device with finger or stylus.
5. The mobile app sends strokes or an exported PNG to the desktop bridge.
6. The desktop bridge writes the same `drawings/latest.svg`, `drawings/latest.png`, and `drawings/latest.json` files used by the native desktop app.
7. User clicks Copy image, uses `/goghmode`, or pastes the generic prompt into the terminal AI interface.

## Why local web app first

- A locally served web app proves the mobile drawing and send-to-desktop loop without iOS or Android packaging, signing, store distribution, hosted backends, or public tunnels.
- The desktop file contract stays stable: Claude Code and other terminal AI tools still inspect `drawings/latest.*`.
- Mobile writes are constrained to the secret local URL path and the configured drawings directory.

## MVP now

- Add a static mobile web app under `mobile/` and embed it into the desktop binary.
- Opening workflow: run `goghmode`, put the phone or iPad on the same Wi-Fi, then use Copy mobile URL or open the `Mobile: http://...` URL shown in the desktop toolbar.
- The desktop app starts a local HTTP server while it is open. The URL includes a persistent random path for local safety and home-screen shortcuts.
- Let the user draw with touch, Apple Pencil, trackpad, or mouse in the browser.
- `Send to Mac` posts the `DrawingSnapshot` shape to the desktop app and writes through `src/export.rs`.
- Keep the output files unchanged: `drawings/latest.svg`, `drawings/latest.png`, and `drawings/latest.json`.
- Keep export buttons for device-local `goghmode-latest.svg`, `goghmode-latest.png`, and `goghmode-latest.json` downloads.
- Do not add accounts, cloud storage, hosted deployment, or public tunnels in this MVP.

## Next slice after MVP

- Add a QR code or shorter local code so opening the phone URL does not require copying or typing a long token.
- Decide whether local-network writes should stay enabled by the secret URL alone or add an extra pairing confirmation in the desktop app.
- Accept a PNG upload path for apps that only send raster images.

## Bridge contract to preserve now

- The desktop app remains the owner of the output directory.
- Mobile input must write through the same export module, not create a parallel file format.
- The prompt and skill must keep pointing at `drawings/latest.*`.
- A failed mobile transfer must not corrupt the last good drawing.

## Open questions for later

- Should mobile pairing use QR code, short code, or manual URL entry?
- Should the mobile app send vector strokes, PNG snapshots, or both?
- Should the desktop bridge expose local-network access by default or require explicit opt-in every session?
