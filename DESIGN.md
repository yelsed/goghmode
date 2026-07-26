---
name: GoghMode
description: A handwriting notebook whose pages are sheets in a drawing set, and whose agent-facing page is the one carrying the issue stamp.
colors:
  sheet: "#FFFFFF"
  sheet-edge: "#D8D4CC"
  register-ground: "#EDEAE4"
  rule: "#A8A29A"
  rule-hair: "#C9C4BB"
  ink: "#1A1917"
  ink-secondary: "#4A4640"
  ink-label: "#6B665E"
  stamp: "#B4331F"
  stamp-review: "#2B5C8A"
  sheet-dark: "#1C1B19"
  sheet-edge-dark: "#3A3733"
  register-ground-dark: "#0E0D0C"
  ink-dark: "#F2EFE9"
  ink-secondary-dark: "#BDB8AF"
  ink-label-dark: "#8E8880"
typography:
  sheet-title:
    fontFamily: "SF Pro Text"
    fontSize: "17px"
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: "normal"
  block-label:
    fontFamily: "SF Pro Text"
    fontSize: "10px"
    fontWeight: 600
    lineHeight: 1.1
    letterSpacing: "0.08em"
  sheet-number:
    fontFamily: "SF Mono"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: 1.1
    letterSpacing: "0.02em"
  stamp:
    fontFamily: "SF Pro Text"
    fontSize: "13px"
    fontWeight: 800
    lineHeight: 1.05
    letterSpacing: "0.10em"
rounded:
  sheet: "2px"
  control: "8px"
spacing:
  hair: "1px"
  block: "8px"
  gutter: "20px"
  margin: "28px"
components:
  register-line:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.ink}"
    padding: "8px 13px"
  title-block:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.ink}"
    padding: "8px 10px"
  block-field-label:
    textColor: "{colors.ink-label}"
    typography: "{typography.block-label}"
  issue-stamp:
    textColor: "{colors.stamp}"
    typography: "{typography.stamp}"
    padding: "6px 10px"
---

# GoghMode — Drawing Set

## Overview

Pages are **sheets in a drawing set**. A drawing set is a numbered sequence where
every sheet carries a ruled title block naming it, and exactly one sheet is stamped
`ISSUED FOR CONSTRUCTION` — the one you build from. Everything else is superseded but
kept.

That maps onto the product without translation: the title block is how a page gets
named, the register is the gallery, and the issue stamp is the page `/goghmode`
reads. The stamp is the only saturated colour in the app, so the question "what is
the agent looking at?" is answerable from across a desk.

The register is the app's home screen. A sheet is somewhere you go and come back
from, so it is pushed and the back button is the only "done" needed; new sheets are
made only where sheets are kept.

Refused, deliberately: the neutral dark thumbnail grid every gallery app ships, and
its opposite, cream-paper notebook skeuomorphism with a serif.

## Colors

Light is primary. The scene is a desk in daylight with an iPad at an angle, and the
drawings themselves are black on white — a dark chrome would fight its own content.
Dark mode is the same set on a light table at night.

- `register-ground` is the surface sheets sit on. Never white; sheets must read as
  objects on top of it.
- `sheet` is pure white and carries the drawing.
- `stamp` (#B4331F) is rubber-stamp ink and is spent **only** on the issued stamp and
  its pin control. It is not a general accent, not a tint for links, not a button
  fill anywhere else.
- `stamp-review` (#2B5C8A) is the lesser stamp for send-now, which marks a sheet as
  sent without moving the pin.
- Text is never grey-on-white by default: `ink-label` is the floor for small labels
  at 4.6:1 on `sheet`, and secondary text uses `ink-secondary` at 8.9:1.

## Typography

SF carries everything, per platform convention. There is no brand face.

- **Title-block labels** are 10pt uppercase, tracked +0.08em — drafting lettering.
  They label fields (`SHEET`, `NAME`, `DATE`, `STROKES`), and never run as prose.
- **Sheet numbers** are SF Mono. Monospace here is measurement and index, not costume:
  numbers must align in a column down the register.
- **Sheet titles** are SF Pro Text semibold at body size, editable in place.
- Dynamic Type scales all of it; the title block reflows to two rows before it clips.

## Layout

- The register is a **ruled index**, not a gallery grid: one sheet per line, on a
  single block of paper laid on `register-ground`, hairline rules between lines.
  A drawing set's register really is a table — sheet number, title, date, status —
  and thirty sheets have to be scannable without scrolling a wall of thumbnails.
- Columns are fixed-width and shared by the head row and every line, so numbers,
  dates and stamps read straight down their columns: `SHEET`, `NAME` (flexible),
  `UPDATED`, `STROKES`, `AGENT`. `UPDATED` and `STROKES` drop out at compact width;
  the rest never move.
- The head row carries the column names in drafting lettering, under a full rule.
- Line height is set by the 40×54 preview, so it never grows with content. Previews
  are small on purpose — enough to recognise a page, not enough to admire it.
- `margin` at the register's edge; the paper block spans the reading width, capped so
  lines never run longer than the eye tracks.

## Elevation & Depth

Elevation is declared **once, as a border** — sheets carry a hairline `sheet-edge`
and no shadow. Paper on paper does not glow. The single exception is the sheet being
dragged during stacking, which lifts on a real offset shadow because it is physically
held.

## Shapes

Sheets are 2px radius — paper corners, not cards. Controls are 8px. Nothing is a
pill except small status chips. No rounded-rectangle placeholders standing in for
content.

## Components

- **Register line** — preview, sheet number, name, facts, stamp control, chevron.
  States: plain, issued (3px `stamp` bar on the leading edge plus the stamp itself),
  dragging (lifts on a real offset shadow, being physically held).
- **Register head** — column names in drafting lettering, above a full rule.
- **Title block** — labelled cells: `SHEET` (mono number), `NAME`, `DATE`,
  `STROKES`. Used where a sheet is presented as an object rather than a line — the
  empty state's blank sheet.
- **Stamp control** — the one control answering "which sheet does the agent read?".
  Unstamped it is a quiet ruled button reading `STAMP`; on the stamped sheet the
  control *is* the stamp, and pressing it lifts the stamp again. Exactly one stamp
  exists across the whole register.
- **Issue stamp** — uppercase, letter-spaced, rotated 2–3°, uneven ink edges,
  arriving with a short impact settle.
- **Series** — a stack. Lettered prefix, offset previews standing in for the sheets,
  count in the `UPDATED` column, sheets numbered within it.
- **Sheet preview** — 40×54, rendered from a source rect that grows to cover the
  drawing. Load-bearing: a portrait-only rect crops away everything drawn on an iPad
  held in landscape, which is a blank preview. Rendered against a light trait because
  the preview sits on paper that stays light in both appearances.

## Do's and Don'ts

- **Do** spend `stamp` red only on the issued stamp. A second red thing on screen
  destroys the one signal the design exists to carry.
- **Do** keep register columns aligned to shared widths. The moment a number drifts
  out of its column the register stops being a register.
- **Don't** let a preview grow large enough to compete with the drawing itself. The
  register is for finding a sheet, not for reading it.
- **Don't** render the register as a stack of cards, or as a plain iOS list with
  `.secondary` grey captions — both forfeit the whole direction.
- **Don't** add blueprint grids, drafting-paper textures, or graph-paper backgrounds.
  The sheet is white; the linework is the user's drawing.
- **Don't** use monospace for anything except numbers and measurements.
- **Don't** introduce shadows on resting sheets, or a border under a shadow.
