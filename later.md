# Later

Deferred work, newest round first. These are product decisions rather than bugs, so they are parked
rather than scheduled.

## Round two, 19 August 2026

From using build 19, the release that brought the flat stamp, swipe-to-delete, zoom, sheet history,
ruling, and `goghmode copy`.

### Waiting on a look, not on code

- **The pressed stamp.** The plan asked for two forms, a rotated stamp kept for the open sheet and a
  flat mark for the register. It shipped with one, flat, everywhere. The rotation was what read as
  ugly inside an aligned column, but the open sheet has room for it and the pressed look is where
  the satisfaction lived. Putting it back is small.
- **The status dot spends stamp red.** `DESIGN.md` reserves that red for the issue stamp, and the
  failed and wrong-host dot breaks it. Predates the drawing set direction. Either the dot loses its
  colour or the rule gains a second sanctioned use, and it is a design call either way. Recorded in
  [the iPad spec](docs/specs/pages/ipad-companion.md) rather than quietly inherited.
- **Ruling could remember the last choice.** Every sheet starts plain, chosen deliberately. If grid
  turns out to be what gets picked every time, defaulting a new sheet to the last ruling used is a
  line of code.

### The trade taken on 19 August

The exported page changes shape when the iPad is rotated. That was given back on purpose when the
fixed portrait page was reverted, because a surface that appeared to stop in the middle of the
screen was the worse problem. It will be felt the first time a sheet drawn in landscape is reopened
in portrait. If that becomes annoying, the third option from that decision is a sheet that remembers
the shape it was first drawn at and keeps it for life.

### Debt

- `UploadController.refusedTheSchema` treats a bare 400 on a ruled sheet as a version refusal, which
  is the only signal the older token route gives. No test covers it; the plan only ever listed it as
  a device check.
- Writing a sheet into the vault as a markdown note, rather than only putting it on the clipboard.
  Needs a name, a folder, and an answer for what happens when the same sheet is sent twice. The
  clipboard route needed none of those, which is why it shipped first.
- The iPad test suite cannot be run on the current development machine. It wedges with no output;
  the same suite runs green on CI in about three minutes. CI is the gate for now.

## Round one, 25 July 2026

Deferred from the first round of iPad companion feedback, parked until the capture-to-agent loop was
boring and reliable. Most of this has since shipped; the headings are kept so the reasoning is not
lost.

The three items from that round that *were* bugs — no eraser, sticky `Offline` status, and unhelpful
error text — are already fixed.

## 1. Multiple pages and a notes overview

**The feedback:** "eigenlijk wil ik meerdere bladzijdes willen kunnen opslaan. dus een overview met
meerdere notes." Doubt about whether written work survives is enough to stop someone writing, so
this is the highest-value deferred item.

**Why it is not a quick fix.** Everything downstream depends on exactly three files being
overwritten in place:

```text
drawings/latest.json
drawings/latest.svg
drawings/latest.png
```

`src/mobile_server.rs` (`handle_save_request`) accepts one snapshot and calls `write_snapshot`,
which always writes to those paths. The `/goghmode` skill reads them. `docs/ai-field-notebook-vision.md`
deliberately calls the latest-page contract stable "because it makes prompting simple".

Adding pages means deciding all of:

- **Does `latest.*` keep meaning "most recently touched page"?** Keeping it is what stops
  `/goghmode` and any other consumer from breaking. Strong default: yes, keep it, and add history
  alongside rather than replacing it.
- **Where does history live?** Something like `drawings/pages/<id>/{json,svg,png}` plus an index,
  versus dated filenames. An index file is easier to list and harder to keep consistent.
- **Who owns page identity — iPad or Mac?** The iPad knows which page the user is on; the Mac owns
  the directory. The snapshot schema currently has no page identifier, so `schemaVersion` would go
  to 2 and `is_valid_snapshot` would need a matching bump.
- **What does the overview actually show?** Thumbnails of local pages on the iPad, or a view of what
  the Mac holds? The second needs a read endpoint, which the server does not have — it only serves
  static assets and accepts `POST /<token>/save`.
- **Deletion and renaming**, which the current write-only design never had to answer.

**Sensible first slice:** page identity in the snapshot, Mac writes history alongside an unchanged
`latest.*`, iPad gets a page switcher over locally-held pages. Defer Mac-side browsing until there
is a reason to read back.

Related open question already recorded in `docs/ai-field-notebook-vision.md`: whether pages are
stored as latest-capture only or as dated history immediately, and whether notebook and whiteboard
are separate modes or one canvas with templates.

## 2. Mac app and iPad app parity

**The feedback:** "ik vind de app op mac een beetje overbodig. maar zie ook de bruikbaarheid.
Kunnen we het iets meer gelijktrekken?"

The observation is fair: once the iPad is the good drawing surface, the Mac canvas is the weaker
way to draw. But the Mac app is not only a canvas — it is the only thing that:

- owns the drawings directory the AI agent reads,
- runs the local server the iPad posts to (`MobileServer`), and
- holds the persistent token in `~/.goghmode/mobile-token`.

So "make them equal" is the wrong frame. The real question is what the Mac app should *become* once
it is no longer the primary drawing surface. Options worth weighing:

- **Keep both canvases, share the toolset.** Most literal reading of the feedback, most work, and
  it argues against PencilKit's advantages being the whole point of the native app.
- **Demote the Mac canvas, promote the bridge.** Mac becomes a connection status window, a page
  browser, and a pairing surface (QR code instead of copy-paste URL), keeping a quick sketch canvas
  for when no iPad is nearby. Fits the vision document's framing of the Mac as "the bridge to the
  AI agent".
- **Headless Mac with a menu bar item.** Smallest surface, but loses the fallback canvas and makes
  pairing awkward.

Leaning toward the second. Decide once multi-page exists, since a page browser is the main thing
that would make the Mac window worth opening.

## 3. Uploads resend the whole drawing

The iPad posts the entire drawing 600 ms after every stroke. Rounding coordinates cut the payload
substantially, but the cost still grows with page length: a long page re-uploads every stroke ever
drawn, every few seconds.

This did not matter while pages were short. It will matter as soon as multi-page notebooks exist,
since a page is meant to be written on for a long time.

Options, cheapest first:

- Skip the upload when the drawing has not changed since the last successful one.
- Send only strokes added since the last acknowledged upload, with the Mac appending. Needs the Mac
  to track per-session state it currently does not have.
- Keep full snapshots but only on an explicit save, with autosave doing deltas.

Not urgent while pages stay short. Revisit with item 1.

## 4. Pairing without copy-paste

Not raised as feedback, but surfaced while fixing the connection bug and worth recording.

`MobileServer::start` (`src/mobile_server.rs`) tries port 8787 and **silently falls back to a random
port** when it is taken. The token in the URL is stable, so a stale URL on the iPad still looks
correct while pointing at a port nothing is listening on. Symptom is an `Offline` badge that no
amount of retrying fixes, with no hint that the address is the problem.

Options, cheapest first:

- Have the Mac app show a warning when it did not get 8787.
- Refuse the random-port fallback and fail loudly instead.
- Show a QR code for the mobile URL so re-pairing is a two-second job and drift stops mattering.

The QR code is the one that makes the problem irrelevant rather than merely visible.

## 5. Obsidian link

**Answered, August 2026: by clipboard, not by writing into the vault.**

`goghmode copy` puts the stamped sheet's PNG on the system clipboard and
`goghmode install-raycast` gives it a hotkey, so one keypress and a paste drops the
drawing wherever the cursor already is: an Obsidian note, a chat window, anywhere
that takes an image. Obsidian files a pasted image into its own attachment folder,
so the vault side needs nothing built.

Writing a sheet into the vault as a markdown note is still open. It needs a name, a
folder, and an answer for what happens when the same sheet is sent twice, and none
of those are needed to paste a picture. The original sketch is kept below.

`docs/ai-field-notebook-vision.md` already sketches the shape: Obsidian as a storage and review layer
rather than a capture surface, with a page becoming a markdown note with metadata plus a linked image
export. Multi-page is what gives it something to link to — a page id is a stable name for a note, which
a single overwritten `latest.*` never was. So this waits on item 1, not on anything else.
