# Desktop Bridge (macOS, Linux)

> Surface: `GoghModeApp` window · Source: `src/app.rs` · Status: done

## Goal & user
Someone at their desk whose iPad is beside them. The window is not where drawing
happens; it is the machine that owns the drawings directory the agent reads, runs
the local server the devices post to, and decides which devices are allowed to.

It has to be open for anything else to work, so its job is to make that state
legible: what has arrived, which sheet the agent is reading, and which devices are
talking to it.

**It stopped being a drawing surface** in July 2026. The canvas, brush, save, undo,
clear, `Send to agent`, `Print prompt` and `Copy image` are gone, and
`tests/app_mobile_url.rs` asserts their absence. Once the iPad is the good drawing
surface, a second, worse canvas competing for the same directory is a liability
rather than a fallback. See
[the design note](../../superpowers/specs/2026-07-26-mac-app-ui-design.md).

## Layout
One window, two views, chosen by a toggle: **Pages** and **Devices**. A status bar
runs along the bottom, and a connection chip sits in the header.

## Components

| Component | Responsibility |
| --- | --- |
| Register (`draw_page_browser`) | Every sheet the host holds, newest first, as cards with a title block. The stamped sheet carries a stamp-coloured border and an `ISSUED` label. |
| Register head | `CLAUDE READS` plus the stamped sheet's name, or "whichever sheet was drawn on last" when nothing is stamped. |
| Devices (`draw_devices`) | Pairing: the QR code, the approval sheet, the paired list with last-seen and last-refusal, host rename, and the legacy-URL toggle. |
| Connection chip | The mobile URL, copyable, for a phone or anything else that cannot pair. |
| Status bar | One line reporting the last write. No modal, no toast. |

## Sub-component specs
- [export-contract](../components/export-contract.md): what the window writes.
- [mobile-server-api](../components/mobile-server-api.md): what it accepts, and why it refuses.

## Design tokens
The Drawing Set palette from [`DESIGN.md`](../../../DESIGN.md), carried in `src/app.rs`
as `SET`. **Light only.** A dark pair was tried and produced two rounds of unreadable
windows; a drawing set is paper, and making that unconditional removed the
disagreement. DESIGN.md's dark tokens stay for whoever does it properly with a device
in front of them.

## Tech used
`eframe` and `egui` 0.34, immediate mode. The window is 1100x760, minimum 720x480,
with `run_and_return: false`.

Thumbnails are decoded from each page's PNG, scaled to 240x160, and cached by
`pageId@updatedAt`, so an edit invalidates its own entry and nothing has to be told
to clear it.

## Auth & permissions
None for the window itself: whoever is at the machine is trusted. The window is where
a person approves a device, which is the trust boundary that matters. See
[ADR-0006](../../decisions/0006-paired-devices-over-shared-url-token.md).

## Data
Owns `~/Pictures/GoghMode/drawings/`, or whatever `--drawings-dir` names. Host
identity, the paired-device registry and the legacy token live in `~/.goghmode/`,
all written `0o600`.

## Client state
`View::{Register, Devices}`, the selected sheet, the pending approval, and the
thumbnail cache. Nothing about drawings is held in memory as truth: the directory is.

## Routes & redirects
None. Single window.

## States

| State | Behaviour |
| --- | --- |
| Serving | Normal. The chip carries a reachable URL, devices can post. |
| Port held by another program | The window says so and serves nothing. Binding 8787 is the lock: moving to another port would leave every saved address pointing at nothing. |
| Already running | The second launch prints the port and exits rather than opening a second window competing for the same directory. |
| Awaiting approval | A device has asked to pair and the sheet is up. The request is blocked on it for up to 60 seconds. |
| Nothing received yet | The register is empty and says how to pair. |

## Estimate
Shipped.

| Scope | Estimate |
| --- | --- |
| Register, thumbnails, stamping | shipped |
| Devices, QR pairing, approval | shipped |
| Canvas removal | shipped |
| Dark appearance | not estimated, deliberately not done |

## Tasks
- [ ] Deletion on the host. The iPad deletes its own copy; the host still only writes.
- [ ] Verify the window actually opens under Hyprland. Carried unverified since the
      Linux support landed.

## Open questions
- Should the window get a dark appearance, given DESIGN.md defines the tokens and the
  first attempt shipped two unreadable windows?
- ~~Should the desktop keep a canvas as a fallback when no iPad is nearby?~~
  **Answered: no.** It competed with the devices for the same directory and was the
  worse surface. Removal is asserted by `tests/app_mobile_url.rs`.
