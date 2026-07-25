# Later

Deferred work from the first round of iPad companion feedback (25 July 2026). These are product
decisions rather than bugs, so they are parked until the capture-to-agent loop is boring and
reliable.

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

## 3. Pairing without copy-paste

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
