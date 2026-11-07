# Hub Receiver Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `goghmode receive --hub <url>` runs without a window, fetches the sheets and stamps the sketch hub queued for this machine, writes them into the drawings directory exactly as a host receiving them directly would, and confirms them.

**Architecture:** A new module `src/hub_receiver.rs` holds the inbox types, applying items with the existing `pages` functions, and a loop. The network is two `curl` calls (fetch and confirm), kept behind two closures so the logic is tested without a network. `main.rs` gets the `receive` subcommand; service files for systemd and launchd ship under `contrib/receiver/`.

**Tech Stack:** Rust 2021, existing crates only; `curl` on the machine (present on Arch and macOS).

**Spec:** dev-portal `docs/superpowers/specs/2026-11-07-sketch-hub-design.md`; contract: dev-portal `docs/sketch-hub-api.md` (PR yelsed/dev-portal#3).

This is plan 2 of 3. Plan 1 (the hub) is built and runs on paperclip at `https://paperclip.procrast.nl:8441`. Plan 3 is the iPad app.

## Global Constraints

- Branch `feature/hub-receiver` from `feature/narrated-sheets` (narrated sheets travel through the hub from day one).
- No new crate (ADR-0004: no HTTP framework, no TLS in the binary); HTTPS goes through the `curl` command.
- A sheet from the hub ends up exactly as `/v2/save` would write it: `pages::write_page`; stamps use `pages::set_pin` and `pages::promote_page`, so pins behave as with a directly paired host.
- The receiver never opens a window and never touches the clipboard.
- Limits and intervals are flags, never constants.
- Descriptive literal names; no em or en dash in copy, comments or docs.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`.

## Review Focus

- The hub is unreachable or answers 403 or 503: the receiver logs one line naming the hub's reason and tries again after the interval; it never exits. Test in Task 2.
- A sheet that fails validation (unsupported schema, unsafe page id): it is confirmed anyway and logged, or it would block the inbox forever. Test in Task 1.
- Writing fails (disk full, permissions): nothing from that item onward is confirmed, so it comes again on the next fetch, in order. Test in Task 1.
- A pin arrives in the same fetch as the sheet it names: the sheet is written first, then the pin, so `latest.*` follows that page. Test in Task 1.
- The drawings directory does not exist yet on a fresh machine: it is created. Test in Task 1.

---

### Task 1: Applying inbox items

**Files:**
- Create: `src/hub_receiver.rs` (with `#[cfg(test)] mod tests`)
- Modify: `src/main.rs` (`mod hub_receiver;`), `src/mobile_server.rs` (make `validate_snapshot` `pub(crate)`)

**Interfaces:**
- Produces:
  - `#[derive(Deserialize)] pub struct InboxItem { pub id: String, pub version: u64, pub kind: String, #[serde(rename = "pageId")] pub page_id: Option<String>, pub snapshot: Option<serde_json::Value> }`
  - `#[derive(Serialize, Debug, PartialEq)] pub struct Confirmation { pub id: String, pub version: u64 }`
  - `pub enum ItemOutcome { Applied(String), Rejected(String) }`: the string is a one-line description for the log
  - `pub fn apply_item(item: &InboxItem, drawings_dir: &Path) -> anyhow::Result<ItemOutcome>`
  - `pub fn apply_inbox(items: &[InboxItem], drawings_dir: &Path) -> (Vec<Confirmation>, Vec<String>)`: applies in order and stops at the first `Err`; returns what to confirm (applied and rejected) and the log lines, the error included.

Kinds: `sheet` parses `snapshot` into `DrawingSnapshot`, runs `validate_snapshot`, then `write_page`; `pin` calls `set_pin(drawings_dir, page_id)`; `promote` calls `promote_page`. A parse or validation failure, an unknown kind, or a promote of a page this machine does not have is `Rejected`; an I/O error is `Err`.

- [ ] **Step 1: Write the failing tests**:
  - `a_sheet_is_written_as_latest`: one sheet item with a minimal valid schema-4 snapshot (reuse the fixture style of `tests/export_snapshot.rs`) into a fresh `tempfile::TempDir` subfolder that does not exist; `latest.json` exists afterwards and the confirmation holds the item's id and version.
  - `a_pin_after_its_sheet_in_one_fetch_points_latest_at_that_page`: sheet page-a, sheet page-b, pin page-a; `pages::read_pin` is `Some("page-a")` and `latest.json` carries page-a's `updatedAt`.
  - `an_invalid_sheet_is_confirmed_and_logged`: `schemaVersion` 99; confirmation contains it, the log line says `unsupported schemaVersion`.
  - `a_write_failure_stops_before_later_items`: drawings dir is a read-only folder (`std::fs::set_permissions` with mode `0o555`, unix only); two items; no confirmations, one log line naming the failure.
  - `an_unknown_kind_is_confirmed_and_logged`.
- [ ] **Step 2: Run** `cargo test hub_receiver`. Expected: FAIL, module missing.
- [ ] **Step 3: Implement** the types and the two functions.
- [ ] **Step 4: Run** `cargo test hub_receiver`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: apply sketch hub inbox items to the drawings directory`.

### Task 2: Fetching and confirming over curl, and the loop

**Files:**
- Modify: `src/hub_receiver.rs`

**Interfaces:**
- Consumes: Task 1.
- Produces:
  - `pub fn fetch_inbox(hub_url: &str, timeout: Duration) -> anyhow::Result<Vec<InboxItem>>`: `curl --silent --show-error --fail-with-body --max-time <secs> <hub_url>/hub/inbox`, parsed from `{"items":[...]}`. A non-zero exit is an error whose message carries the hub's `{"error":...}` text when the body has one.
  - `pub fn confirm(hub_url: &str, timeout: Duration, confirmations: &[Confirmation]) -> anyhow::Result<()>`: `POST <hub_url>/hub/inbox/confirm` with body `{"items":[...]}` on stdin (`--data-binary @-`, `Content-Type: application/json`). Skipped when the list is empty.
  - `pub fn receive_once(fetch: impl Fn() -> anyhow::Result<Vec<InboxItem>>, confirm: impl Fn(&[Confirmation]) -> anyhow::Result<()>, drawings_dir: &Path) -> anyhow::Result<Vec<String>>`
  - `pub fn run_receiver(hub_url: &str, drawings_dir: &Path, interval: Duration, timeout: Duration) -> !`: loops `receive_once`, prints each log line with a timestamp, prints errors to stderr, sleeps `interval`, never returns.

- [ ] **Step 1: Write the failing tests**:
  - `receive_once_confirms_what_it_applied`: fake fetch returns one valid sheet; fake confirm records its argument; recorded is `[Confirmation{id, version}]`.
  - `receive_once_reports_a_fetch_error_and_confirms_nothing`: fetch returns `Err("paperclip is not allowed on the sketch hub")`; the result is that error and confirm was never called.
  - `fetch_inbox_reads_items_from_a_hub` and `fetch_inbox_carries_the_hubs_reason_on_403`: a `std::net::TcpListener` on `127.0.0.1:0` in a thread answers once with a canned HTTP/1.1 response (200 with `{"items":[...]}`, then 403 with `{"error":"ipad148 is not allowed on the sketch hub"}`); the URL is `http://127.0.0.1:<port>`. Skip the two tests with a printed note when `curl` is not on `PATH`.
- [ ] **Step 2: Run** `cargo test hub_receiver`. Expected: FAIL.
- [ ] **Step 3: Implement** the four functions with `std::process::Command`.
- [ ] **Step 4: Run** `cargo test hub_receiver`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: fetch and confirm the sketch hub inbox with curl`.

### Task 3: The `receive` subcommand, service files and docs

**Files:**
- Modify: `src/main.rs`, `README.md`
- Create: `contrib/receiver/goghmode-receive.service`, `contrib/receiver/goghmode-receive.plist`

**Interfaces:**
- Consumes: Task 2 `run_receiver`.

`Command::Receive { #[arg(long)] hub: String, #[arg(long, default_value_t = 5)] interval_seconds: u64, #[arg(long, default_value_t = 20)] timeout_seconds: u64 }`, using the global `--drawings-dir` or `default_drawings_dir()`. The systemd user unit runs `%h/.cargo/bin/goghmode receive --hub https://paperclip.procrast.nl:8441` with `Restart=always`, `RestartSec=10`; the launchd agent runs the same through `/bin/sh -c 'exec "$HOME/.cargo/bin/goghmode" receive --hub https://paperclip.procrast.nl:8441'` with `KeepAlive` and `RunAtLoad`. README section "Receive through the sketch hub": what it does, the one-time allow step on `https://dev.procrast.nl/`, install commands for both platforms, and that pins behave as with a paired host.

- [ ] **Step 1: Write the failing test** `receive_needs_a_hub_url` in `src/main.rs` tests: `Cli::try_parse_from(["goghmode", "receive"])` is an error; with `--hub https://example` it parses with interval 5 and timeout 20.
- [ ] **Step 2: Run** `cargo test receive_needs`. Expected: FAIL.
- [ ] **Step 3: Implement** the subcommand, the two service files and the README section.
- [ ] **Step 4: Run** the gates: `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`. Expected: all pass.
- [ ] **Step 5: Run on paperclip by hand** (ask the owner before installing or starting a service): `cargo build`, then `timeout 30 target/debug/goghmode --drawings-dir <scratch dir> receive --hub https://paperclip.procrast.nl:8441 --interval-seconds 5`. Expected: one line per attempt with the hub's 403 reason ("paperclip is not allowed..."). After the owner allows `paperclip` on the page and a sheet is queued with curl (`docs/sketch-hub-api.md` example), the same run writes `latest.json` and the hub reports the page as `delivered`.
- [ ] **Step 6: Commit** `feat: goghmode receive, with service files for systemd and launchd`.
