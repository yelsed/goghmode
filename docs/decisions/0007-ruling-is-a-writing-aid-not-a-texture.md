# ADR-0007 · Ruling is a writing aid, not a texture

- **Status:** Accepted
- **Date:** 2026-08-18

## Context

DESIGN.md shipped with a flat refusal: "Don't add blueprint grids, drafting-paper
textures, or graph-paper backgrounds. The sheet is white; the linework is the
user's drawing."

That rule was written against decoration. A blueprint grid behind someone's
handwriting is chrome dressed as content, and the drawing set direction exists
partly to refuse exactly that kind of costume.

Using the app daily surfaced a different need. Handwriting stays straight and a
diagram keeps its proportions much more easily against a rule than against blank
white. This is the same reason paper notebooks are sold lined and squared. Asking
for it is not asking for texture, it is asking for the aid.

Two things had to be settled alongside it:

- **Whose choice is it?** A single application-wide preference would mean a lined
  note and a squared diagram cannot exist side by side, which is the normal case.
- **Does the agent see it?** The exported PNG is what an agent reads. A page that
  looks one way on the iPad and another in `latest.png` is two different pages.

## Decision

Ruling is a per-sheet property, stored with the page, `plain` by default, and it is
drawn into the exported PNG and SVG under the ink.

- Four values: `plain`, `lines`, `grid`, `dots`, with one spacing in page units.
- It is only ever drawn inside the drawing area. It is never chrome, never a
  background for a screen, never behind the register.
- Ruling ink is fixed in the exporter at `rule-hair` (`#C9C4BB`) and is not sent by
  the client, so nothing on the network can put arbitrary marks into the file the
  agent reads.
- The register preview keeps showing the drawing, not the ruling.
- Carrying it needs a wider snapshot, so this is schema version 3. Versions 1 and 2
  keep working unchanged, and a sheet with no ruling exports byte-for-byte as it
  did before.

## Consequences

- DESIGN.md's blanket refusal is replaced by a narrower one: no ruling by default,
  and never outside the drawing area. The thing it was protecting against, texture
  as decoration, is still refused.
- A third schema version is now in circulation. The companion asks the host what it
  accepts and sends version 2 without ruling to a host that predates this, so an
  un-updated Mac keeps receiving drawings.
- The exported page and the sheet on the iPad now agree, which they did not have to
  before.
- An agent reading a ruled page will see the rules and may describe them. That is
  the correct trade: it is describing the page that was drawn on.

## Alternatives considered

- **On-screen only, plain export.** Smaller change, no schema bump. Rejected because
  the exported page would stop being the page that was drawn on, and the export is
  the whole product.
- **One application-wide ruling preference.** No schema change at all. Rejected
  because a lined note and a squared diagram are the ordinary case, not an edge one.
- **Keep the refusal and do nothing.** Rejected: the rule was aimed at decoration
  and was being applied to a writing aid, which is a different thing.
