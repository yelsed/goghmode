# Narrated sheets: what was said while each part was drawn (design, 19 September 2026)

## Context

A whiteboard explanation is easy to follow live and unreadable afterwards: everything ends up on
top of everything else. GoghMode already separates strokes and knows when each was made, so the
sheet has a timeline. The idea is to record speech on the iPad while drawing, transcribe it on the
device, and let the host export a package that ties each spoken sentence to the ink that was added
while it was said. An agent (Claude Code, Codex) then reads the sheet the way a person watched it
being drawn: step by step, with the words. The user's own summary of the value: speech and drawing
give each other context; that coupling is the win, not transcription on its own.

Decisions taken with the user on 19 September 2026:

- **Package for the agent:** a markdown timeline plus one small PNG crop per step, showing only the
  region where ink was added. New ink is pointed at without changing the stroke itself (no
  recolouring, no width change). Full-page frames were dropped: every image an agent reads costs
  tokens (a full page is roughly 1 900), and the user wants a small image footprint with no hard
  cap on the number of steps.
- **Engine:** WhisperKit on the iPad, from day one. Reason: Dutch quality, word/segment timing, and
  the path to a model fine-tuned on the user's own voice later (whisperkittools). Apple's
  SpeechAnalyzer was considered (free, on-device, Dutch, iPadOS 26+) and rejected because it
  cannot be personalised.
- **Recording:** one on/off button in the open sheet's toolbar. Recording belongs to that sheet and
  stops when the sheet is closed or the app goes to the background.
- **Audio:** kept on the iPad beside the sheet as 16 kHz mono WAV, never sent to the host. This is
  the training data for a later "learns how I speak" model; without it there is nothing to learn from.

Facts that shape the design (verified in the repo and upstream docs):

- Wire format is at `schemaVersion` 3 (ruling). `src/drawing.rs:80` and `:83`. Server accepts
  `{1,2,3}` (`src/mobile_server.rs:29`), capabilities advertise them (`:34`). A sheet asks for the
  highest version only when it carries the feature, and falls back on a refusal
  (`UploadController.refusedTheSchema`, `ipad-companion/.../UploadController.swift:383`).
- iPad `point.t` is a per-stroke offset (`DrawingSnapshot.swift:167`), not a clock. Stroke ids are
  index-based and change when an earlier stroke is erased (`:174`). Alignment must therefore use an
  absolute per-stroke start time, not ids. PencilKit gives one: `PKStrokePath.creationDate`.
- All writes go through `export::write_artifacts` (`src/export.rs:144`); `pages::write_page` mirrors
  a page to `latest.*` unless another page is pinned (`src/pages.rs:119`). `StoredPage`
  (`src/pages.rs:64`) repeats the snapshot fields by hand and must gain any new field.
- The `/goghmode` skill text lives in `src/skill.rs:8`; installed copies can be older than the app,
  so `latest.*` must keep meaning what it means.
- The Xcode project has no Swift package dependencies yet (`project.pbxproj`), deployment target
  iOS 17, `Info.plist` has camera and local-network usage strings, no microphone string.
- WhisperKit now ships from `https://github.com/argmaxinc/argmax-oss-swift` (product `WhisperKit`,
  platforms iOS 16+). API used: `WhisperKit(WhisperKitConfig(model:))`,
  `WhisperKit.download(variant:progressCallback:)`, `WhisperKit.recommendedModels()`,
  `audioProcessor.startRecordingLive(callback:)` (sets `AVAudioSession` to `.playAndRecord`
  itself), `audioProcessor.audioSamples` (16 kHz `Float`), `transcribe(audioArray:decodeOptions:)`
  with `DecodingOptions(language:, chunkingStrategy: .vad)`; results are `TranscriptionSegment
  { start, end, text }` in seconds from the start of the audio. Model folders on
  `argmaxinc/whisperkit-coreml` include `openai_whisper-large-v3-v20240930_626MB` (M1 and newer
  iPads, also A15+); older iPads get `openai_whisper-base` as default.
- Procrastination-station uses whisper.cpp through flutter_rust_bridge with `small`/`medium` for
  Dutch. Not reusable in a Swift app, but it confirms `tiny` is useless for Dutch and that
  batch transcription after stop is a workable shape.

## Design

### 1. One clock, two tracks

Everything is stamped in **epoch milliseconds from the iPad's own clock**, so the host only sorts.

- `stroke.startedAt` (new, optional `u64`): `PKStrokePath.creationDate` of the stroke. Sent by the
  iPad on every stroke once it speaks version 4. Points keep their per-stroke `t`.
- `narration` (new, optional object on the snapshot):

```jsonc
"narration": {
  "language": "nl",
  "engine": "whisperkit/openai_whisper-large-v3-v20240930_626MB",
  "segments": [
    { "start": 1758290000000, "end": 1758290003400, "text": "Dit is de database." }
  ]
}
```

Segment times are `recordingStartedAt + segment.start/end` from WhisperKit. Several recordings on
one sheet simply append segments. Erased strokes vanish from the snapshot; their words stay.

### 2. Schema version 4

`NARRATED_SCHEMA_VERSION = 4`, `CURRENT_SCHEMA_VERSION = 4`. Same rule as ruling: a sheet asks for
4 **only when it carries narration**, so nothing changes for a sheet nobody spoke over, and an old
host still gets the strokes. `startedAt` is accepted at any version (it is an annotation, and old
hosts ignore unknown fields; nothing visible is lost when it is dropped). `narration` at a version
below 4 is refused with a named reason.

Server (`src/mobile_server.rs`): `SUPPORTED_SCHEMA_VERSIONS = [1,2,3,4]`; capabilities become
`{"schemaVersions":[1,2,3,4],"features":[...,"narration"]}`; `validate_snapshot` adds: narration
needs version 4; `language` ≤ 16 chars; `engine` ≤ 128; ≤ 4096 segments; each `start <= end`, text
≤ 2000 chars; `startedAt`, when present, finite. Body limit (4 MiB) is untouched: half an hour of
speech is well under 100 KB.

### 3. What the host writes (the package)

`export::write_artifacts` grows two outputs, written only when the snapshot has narration:

```text
drawings/latest.timeline.md            drawings/pages/<id>/page.timeline.md
drawings/latest.steps/001.png …        drawings/pages/<id>/page.steps/001.png …
```

`latest.json` / `page.json` gain `narration`, `startedAt` per stroke, and `files.timeline` /
`files.steps` entries when present. `latest.{json,svg,png}` keep their meaning byte for byte for
an unnarrated sheet.

**Steps.** Sort segments by `start`, strokes by `startedAt`. Each stroke belongs to the segment
with the greatest `start <= startedAt`; strokes before the first word form a leading step with no
text. A segment that added no ink merges its text into the previous step, so every step has ink
and every sentence is kept. No practical cap: only above `MAX_STEPS = 200` are neighbouring steps
merged evenly, and the markdown still quotes every sentence.

**Step crops.** One small PNG per step, showing only where ink was added: the bounding box of the
step's strokes (their widths included), padded by 24 page units, clamped to the page, scaled so
the long side is at most 512 px. Drawn in this order, all within that window: white sheet, ruling,
then a **halo** under every stroke added in this step (the stroke's own path at radius + 6 page
units, in a pale blue derived from `stamp-review`, `#D6E4F0`), then every earlier stroke that
enters the window in its own colour and width, then this step's strokes in their own colour and
width. Nothing about any stroke changes; the halo sits under all ink, so earlier ink crossing the
halo stays legible, and going back to an old region reads correctly because the halo follows the
strokes, not a region. The scale is applied to the geometry before rasterising (translate by the
window origin, multiply coordinates and radii), never by resampling a bigger image, so every pixel
is one of a handful of exact colours and the file stays tiny. Encoded as an **8-bit palette PNG**
through the `png` crate that `image` already pulls in (palette: white, ruling ink, halo, stroke
colours; falls back to `image`'s Rgb8 encoder if a step somehow needs more than 256 colours).
Expected size 5–20 KB per step, roughly 150 tokens when an agent reads one.

**Timeline markdown.** Header: sheet title, language, step and segment counts, total span, the
final PNG path, and two sentences: one explaining the halo, one saying each crop is a window on
the page whose position the step line gives. Then per step:

```markdown
## Step 3 · 00:41–01:05 · latest.steps/003.png
> Dit is de database, en hier komt de API die ermee praat.
Drawn: 6 strokes, top-left · window x 56–434, y 36–274 on a 1024×1366 page.
```

Times are mm:ss from the earliest of first stroke and first word. The "Drawn" line gives the
count, a coarse region word from the window's centre (`top-left`, `centre`, `bottom-right`, …),
and the window itself in page units.

**Atomicity and staleness.** Crops are rendered into `<stem>.steps.tmp/`, then the old directory
is renamed aside, the new one renamed into place, and the old one removed. The markdown goes
through the existing `.tmp` + rename. When a sheet **without** narration is written to `latest`,
any `latest.timeline.md` and `latest.steps/` are removed, so the agent can never read yesterday's
words against today's sketch. The same holds per page directory.

**A second skill, not a bigger one.** `/goghmode` stays exactly as it is: the app must keep
working for people who never speak over a sheet, and an installed skill can be older than the
app. `src/skill.rs` gains a second embedded skill, `goghmode-narrated`, written to
`~/.claude/skills/goghmode-narrated/SKILL.md` by the same `goghmode install-skill --target claude`
run (one command installs both). Its description triggers on "what did I say while drawing",
"walk me through this sketch", "the narrated sheet", "the explanation on the whiteboard". Steps:
read `latest.timeline.md`; if it is missing, say the current sheet was not narrated and hand over
to `/goghmode`; otherwise read the steps in order, opening the step crops it names (small on
purpose), then `latest.png` for the whole page; judge age from `updatedAt` in `latest.json` the
same way `/goghmode` does. It explains the halo in one sentence and repeats the two-locations rule
(`~/Pictures/GoghMode/drawings` and a project-local `drawings/`). `goghmode prompt` and
`src/prompt.rs` are left untouched.

### 4. iPad: recording, transcribing, keeping the audio

New `NarrationRecorder` (`@MainActor final class ... ObservableObject`), one per open sheet:

- `state`: `idle`, `preparingModel(progress)`, `recording(since: Date)`, `transcribing`,
  `failed(String)`.
- `start()`: `AudioProcessor.requestRecordPermission()`; if no model is loaded, pick the model
  (`WhisperKit.recommendedModels()`; prefer `openai_whisper-large-v3-v20240930_626MB` when
  supported, otherwise the device default), `WhisperKit.download` with progress, load; then
  `audioProcessor.startRecordingLive` and record `recordingStartedAt = Date()`. Each buffer
  callback appends to an `AVAudioFile` (16 kHz mono) at
  `Application Support/goghmode-narration/<pageID>/<startedAtMs>.wav`, so a killed app loses
  nothing recorded so far.
- `stop()`: `stopRecording()`, close the file, transcribe `audioProcessor.audioSamples` with
  `DecodingOptions(task: .transcribe, language: "nl", temperature: 0, chunkingStrategy: .vad)`,
  map segments to epoch milliseconds, trim text, drop empties, hand `[NarrationSegment]` to the
  caller. Wrapped in `UIApplication.beginBackgroundTask` so leaving the sheet does not kill it.
- A WAV without a `.json` sidecar (transcription never finished) is transcribed the next time that
  sheet is opened, and the sidecar is written when done. This is the crash-recovery path.
- Language is the constant `"nl"` for now; a setting comes later.
- Model download happens once, on first use, from HuggingFace; the control shows the percentage.

`PageStore` / `NotebookPage`: `var narration: [NarrationSegment]?` (decodes as `nil` for older
stores, same pattern as `ruling`), `appendNarration(_:on:)`, and `delete` also removes the page's
audio directory.

`DrawingSnapshot.fromPencilDrawing(..., narration:)`: fills `startedAt` from
`pencilStroke.path.creationDate`, attaches `narration` when non-empty, asks for
`narratedSchemaVersion = 4` in that case. `withoutNarration()` mirrors `withoutRuling()`.

`UploadController`: `narrationSupported` flag and `narrationUnsupportedMessage`, learned from a
refusal exactly as ruling is (`refusedTheSchema` already matches on `schemaVersion`); on refusal the
sheet is re-sent without narration and the notice line says the desktop is an older version that
cannot keep what was said. After a transcription lands, the sheet is uploaded at once
(`uploadNow`), not on the next stroke.

`CanvasView` toolbar, between the stamp and undo: `NarrationControl`. Idle: `mic` symbol. Preparing:
`mic` plus percentage in mono caption. Recording: `waveform` symbol plus `mm:ss` in mono caption,
tinted `Sheet.review` (the lesser blue; stamp red stays reserved). Transcribing: `mic.badge.xmark`
replaced by `text.bubble` with a progress spinner. Failure goes to the existing notice line through
`uploader.complaint`'s sibling `recorder.complaint`. The control keeps one width in every state,
like `StatusBadge`. `onDisappear` and `scenePhase == .background` call `recorder.stop()` before the
existing upload.

`Info.plist`: `NSMicrophoneUsageDescription` ("GoghMode records what you say while you draw, and
turns it into text on this iPad. Audio stays on the device."). No background audio mode: recording
stops when the app leaves the foreground, by design.

Package dependency: `argmax-oss-swift` (product `WhisperKit`) added to `project.pbxproj` as an
`XCRemoteSwiftPackageReference` pinned to a release, `Package.resolved` committed. CI
(`xcodebuild test` / archive) resolves packages on its own.

### 5. Delivery, in shippable slices

1. **Host first** (Rust, no client sends v4 yet): schema, validation, capabilities, timeline and
   step crops, stale-file removal, the second skill, tests, docs (export contract, server API,
   ARCHITECTURE wire contract, OVERVIEW feature entry, README install and skill sections,
   ADR-0008).
2. **iPad**: package dependency, `NarrationRecorder`, audio files and recovery, `PageStore`
   narration, snapshot v4 and fallback, toolbar control, Info.plist, Swift tests, ipad-companion
   spec update.
3. **Later, not now**: a "narrated" fact in both registers; language setting; live caption line
   while recording; fine-tuning pipeline that reads the kept WAVs.

## Files

### Rust host (slice 1)
- `src/drawing.rs`: `Stroke.started_at`, `Narration`, `NarrationSegment`, `DrawingSnapshot.narration`,
  `NARRATED_SCHEMA_VERSION`, bump `CURRENT_SCHEMA_VERSION` to 4.
- `src/mobile_server.rs`: `SUPPORTED_SCHEMA_VERSIONS`, `CAPABILITIES`, `validate_snapshot` additions.
- `Cargo.toml`: declare `png` directly (same crate `image` already compiles) for palette encoding.
- `src/export.rs`: `ExportJson` fields; step building, merging and markdown in a new module
  `src/timeline.rs`; crop rendering as `render_step_crop(snapshot, step) -> RgbaImage` beside
  `snapshot_to_rgba`, sharing `fill_brush`/`draw_segment` through a small window transform
  (origin, scale); palette encoding in one function; `write_artifacts` writes/removes
  `<stem>.timeline.md` and `<stem>.steps/`.
- `src/pages.rs`: `StoredPage` gains `started_at` per stroke and `narration`.
- `src/skill.rs`: second embedded skill `NARRATED_SKILL`, `skill_path` gains the second location,
  `install_skill` writes both files and returns both paths; `src/main.rs` prints both. The
  existing `CLAUDE_SKILL` text is not changed. `src/prompt.rs` untouched.
- `tests/export_snapshot.rs`, `tests/mobile_server.rs`, `tests/paired_uploads.rs`,
  `tests/skill_install.rs`, `tests/prompt.rs`: new cases (below).
- Docs: `docs/specs/components/export-contract.md`, `docs/specs/components/mobile-server-api.md`,
  `docs/ARCHITECTURE.md` (wire contract, module table), `docs/OVERVIEW.md`,
  `docs/decisions/0008-narration-is-transcribed-on-the-device.md`, `docs/PLANNING.md`,
  `docs/superpowers/specs/2026-09-19-narrated-sheet-design.md` (this design, as the spec).

### iPad companion (slice 2)
- `GoghModeCompanion.xcodeproj/project.pbxproj` (+ `Package.resolved`): WhisperKit.
- `GoghModeCompanion/NarrationRecorder.swift` (new), `NarrationSegment` in `DrawingSnapshot.swift`.
- `DrawingSnapshot.swift`: `startedAt`, `narration`, `narratedSchemaVersion`, `withoutNarration()`.
- `PageStore.swift`: `NotebookPage.narration`, `appendNarration`, audio directory removal on delete.
- `UploadController.swift`: `narrationSupported`, message, refusal fallback.
- `ContentView.swift`: `NarrationControl`, recorder lifecycle, notice line.
- `Info.plist`: microphone usage string.
- `GoghModeCompanionTests/DrawingSnapshotTests.swift`, `PageStoreTests.swift`, new
  `NarrationRecorderTests.swift` (pure mapping only).
- `docs/specs/pages/ipad-companion.md`.

## Verification

Rust, all local (`cargo test`, `cargo clippy`):
- A narrated snapshot written through `write_page` produces `page.timeline.md` and
  `page.steps/NNN.png` with one crop per step, mirrors them to `latest.*`, and `latest.json.files`
  names them.
- Crop pixels: a stroke drawn in step 2 has halo-coloured pixels around it in crop 2 and the crop
  for step 1 does not contain it; its own ink pixels are the stroke's colour; a crop's long side is
  at most 512 px and every crop decodes as an indexed PNG (checked by reading the IHDR colour type).
- A step whose strokes span the whole page still yields a crop no larger than 512 px on its long
  side; a single dot yields a crop of at least the padding.
- 300 segments each with ink stay 300 steps and 300 crops; 250 steps with `MAX_STEPS` forced to 40
  in the test collapse to 40, and the markdown still has 250 quotes.
- A plain sheet written to `latest` after a narrated one removes `latest.timeline.md` and
  `latest.steps/`.
- No `.tmp` files or `.steps.tmp` directories remain after a write.
- `validate_snapshot`: narration at version 3 → 400 naming `schemaVersion`; `start > end` → 400;
  version 4 with valid narration → 200 and files written; capabilities JSON lists 4 and `narration`.
- `install_skill` writes `~/.claude/skills/goghmode/SKILL.md` byte-identical to before and a new
  `~/.claude/skills/goghmode-narrated/SKILL.md`; the new text names `latest.timeline.md`, the
  `latest.steps/` directory, both drawings locations, and the hand-over to `/goghmode` when no
  timeline exists; neither text contains shell metacharacters (extend the existing test to both).
  The existing `/goghmode` assertions in `tests/skill_install.rs` and `tests/prompt.rs` pass
  unchanged.

Swift (`xcodebuild test` on CI, since the suite wedges on the development machine):
- JSON key shape for `startedAt` and `narration` matches the Rust struct; a narrated snapshot asks
  for version 4 and `withoutNarration()` drops to 3 or 2 correctly.
- Mapping of WhisperKit seconds to epoch milliseconds, trimming, and empty-segment dropping.
- A store written without `narration` decodes with `nil`; `appendNarration` persists.

On device, by hand: record 30 s while drawing three groups of strokes in three regions; check the
Mac writes three crops, each showing its own region with the halo under the new ink; erase a
stroke and confirm the words survive; close the sheet mid-transcription and confirm the text
arrives after reopening; pair with a host built before slice 1 and confirm the sheet still arrives
plain with the notice. Then run `/goghmode-narrated` in Claude Code on the narrated sheet and
check it reads the timeline first and describes the steps in order, and run `/goghmode` on the
same sheet and check it behaves exactly as before.

## Open items carried into implementation
- Exact SF Symbols and the fixed width of `NarrationControl` are settled in code against the
  existing `StatusBadge`.
- Halo radius (+6 page units), padding (24), crop long side (512 px), `MAX_STEPS` (200) and the halo
  colour are named constants in `src/export.rs` / `src/timeline.rs`, tuned once real crops are
  viewed.
