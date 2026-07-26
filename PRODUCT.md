# Product

<!-- impeccable:product-schema 1 -->

## Platform

ios

The iPad companion is the surface this record governs. GoghMode also ships a macOS
desktop app (Rust + egui) that owns the output directory and runs the local server;
it is a second surface of the same product, not a second product.

## Users

One developer, at a desk with a Mac running Claude Code and an iPad with an Apple
Pencil beside it. The job is thinking on paper — handwriting, diagramming, sketching
a layout or a system — and then having the agent on the Mac read it without a photo,
an AirDrop, or a file hunt.

Writing happens in short bursts, mid-conversation with the agent. The iPad is picked
up, written on, and put down. It is not a session someone sits down to start.

## Product Purpose

Remove the distance between handwriting something and an AI agent being able to read
it. A physical notebook is faster to write in than any app; sharing it is the part
that fails. GoghMode exists to make sharing automatic, so the notebook stays worth
using.

Success is that the user writes by hand *more often* because reaching the agent costs
nothing — and never hesitates because they doubt the page will still be there.

## Positioning

The drawing surface is native PencilKit, but the product is the bridge. A neighbouring
sketch app syncs to a cloud account; GoghMode writes plain files into a directory on
your own Mac that a local agent already knows how to read. No account, no cloud, no
tunnel. The Mac is the server, and it is on your desk.

## Operating Context

- Mac and iPad on the same Wi-Fi. The Mac runs the app; the iPad posts to it over the
  LAN at `http://<lan-ip>:8787/<token>/`.
- Pairing is manual: the Mac shows a URL, the user pastes it into the iPad once.
- The agent reads `drawings/latest.{json,svg,png}` through the `/goghmode` Claude Code
  skill, which is installed into `~/.claude/skills/` and can be an older copy than the
  running app.
- Three drawing clients exist and cannot be updated simultaneously: the iPad app, the
  macOS canvas, and a browser companion served by the Mac.

## Capabilities and Constraints

- **Pages.** Every save keeps its own copy under `drawings/pages/<pageId>/`;
  `latest.*` mirrors whichever page was written most recently. Page ids are minted by
  the client and become directory names, so the server restricts them to
  `[A-Za-z0-9_-]{1,64}`.
- **Naming.** Pages are titled. Titles default to a timestamp and are user-editable.
- **Stacks.** Grouping is iPad-local: pages can be stacked and a stack is named. The
  wire format and the Mac's flat `pages/` directory are deliberately unchanged by it.
- **Pinning.** A pinned page owns `latest.*` until another is pinned, so the agent's
  view stops drifting to whatever was touched last. A separate send-now action pushes
  any page immediately without moving the pin.
- **Wire format.** `schemaVersion` 1 and 2 are both accepted; 2 carries a page. The
  Mac advertises what it accepts at `GET /<token>/capabilities`, and a client that
  gets a 404 there falls back to version 1.
- **Retention.** Pages are kept forever. Nothing expires them on a timer.
- Vector strokes are sent, never PNG uploads, so the Mac owns rendering.
- Non-goals, standing: hosted accounts, cloud sync, public tunnels, replacing Obsidian.
- Distribution is TestFlight via GitHub Actions. iOS 17 minimum, universal iPad and
  iPhone.

## Brand Commitments

The name GoghMode. No logo, wordmark, or palette has been committed — the current
look is default SwiftUI chrome and carries no identity worth preserving.

## Evidence on Hand

- `docs/ai-field-notebook-vision.md` — product direction, capture modes, non-goals.
- `later.md` — deferred work with the original Dutch user feedback quoted verbatim.
- `docs/specs/` — component and page specs, including the export contract and server
  API.
- No users beyond the author, no usage data, no testimonials. Nothing may imply
  otherwise.

## Product Principles

1. **Capture beats organization.** Opening to a usable pen matters more than any
   filing system. Organization is something the user does later, if at all.
2. **Never make someone doubt their work survived.** This is the anxiety that stops
   people writing; every design decision answers to it.
3. **The agent's view is explicit, not incidental.** Which page the agent reads is a
   thing the user chooses, not a side effect of what they last touched.
4. **The bridge is local and legible.** Plain files in a directory the user owns,
   readable without this app.
5. **Old clients keep working.** Three implementations ship at different speeds; the
   contract widens rather than breaks.

## Accessibility & Inclusion

- Text must meet WCAG AA contrast against its actual background. The current settings
  screen fails this — secondary grey label text on a white ground — and it is a known
  defect, not a style choice.
- The app is used one-handed with a Pencil in the other hand, often at an angle and in
  variable desk light. Targets and contrast are judged in that scene, not head-on at
  full brightness.
- Dynamic Type must not clip page titles or stack names.
