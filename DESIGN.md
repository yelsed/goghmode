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
  sheet-card:
    backgroundColor: "{colors.sheet}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sheet}"
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

- The register is a grid of sheets on `register-ground`, two columns on iPad portrait,
  three on landscape, one on iPhone.
- A sheet is drawing area above, title block below, full width, no gap between them —
  the block is part of the sheet, not a caption under a card.
- The title block is a ruled form: hairline rules divide labelled cells, and cells
  align across every sheet in the register so the eye can read down a column.
- More space above a heading than below it; `gutter` between sheets, `margin` at the
  register's edge.

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

- **Sheet card** — drawing + title block. States: plain, issued (stamped), sent,
  stacked (offset sheet edges visible behind), empty (ruled but unfilled block).
- **Title block** — labelled cells: `SHEET` (mono number), `NAME` (editable),
  `DATE`, `STROKES`.
- **Issue stamp** — uppercase, letter-spaced, rotated 2–3°, uneven ink edges,
  arriving with a short impact settle. Exactly one exists across the whole register.
- **Series** — a stack. Lettered prefix, cover sheet showing count, sheets numbered
  within it.

## Do's and Don'ts

- **Do** spend `stamp` red only on the issued stamp. A second red thing on screen
  destroys the one signal the design exists to carry.
- **Do** make the title block an actual ruled form with visible cell divisions.
- **Don't** render the title block as a plain caption row — that is the lazy version
  and it forfeits the whole direction.
- **Don't** add blueprint grids, drafting-paper textures, or graph-paper backgrounds.
  The sheet is white; the linework is the user's drawing.
- **Don't** use monospace for anything except numbers and measurements.
- **Don't** introduce shadows on resting sheets, or a border under a shadow.
