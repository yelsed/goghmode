//! The receiving end of the dev-portal sketch hub: fetch what the hub queued
//! for this machine, write it the way a host writes a direct upload, confirm it.
//! The contract lives in dev-portal's `docs/sketch-hub-api.md`.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::drawing::DrawingSnapshot;
use crate::mobile_server::validate_snapshot;
use crate::pages::{load_page, page_id_is_safe, promote_page, set_pin, write_page};

/// One queued delivery as the hub hands it out, in send order.
#[derive(Debug, Deserialize)]
pub struct InboxItem {
    pub id: String,
    pub version: u64,
    pub kind: String,
    #[serde(rename = "pageId")]
    pub page_id: Option<String>,
    #[serde(default)]
    pub snapshot: Option<serde_json::Value>,
}

/// What the hub needs back to drop an item: the version fetched, so a newer
/// upload of the same page that arrived meanwhile stays queued.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Confirmation {
    pub id: String,
    pub version: u64,
}

pub enum ItemOutcome {
    Applied(String),
    /// Can never succeed; confirmed anyway so it cannot block the queue (ADR-0010).
    Rejected(String),
}

/// Applies one item with the host's own functions. An `Err` is a failure that
/// may pass (the disk, permissions), so the item must come again.
pub fn apply_item(item: &InboxItem, drawings_dir: &Path) -> anyhow::Result<ItemOutcome> {
    std::fs::create_dir_all(drawings_dir)?;
    match item.kind.as_str() {
        "sheet" => {
            let Some(raw) = &item.snapshot else {
                return Ok(ItemOutcome::Rejected(format!(
                    "{} carries no snapshot",
                    item.id
                )));
            };
            let snapshot = match DrawingSnapshot::deserialize(raw) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    return Ok(ItemOutcome::Rejected(format!(
                        "{} is not a drawing: {error}",
                        item.id
                    )))
                }
            };
            if let Err(reason) = validate_snapshot(&snapshot) {
                return Ok(ItemOutcome::Rejected(format!(
                    "{} set aside: {reason}",
                    item.id
                )));
            }
            write_page(&snapshot, drawings_dir)?;
            Ok(ItemOutcome::Applied(format!("wrote {}", item.id)))
        }
        "pin" => {
            if let Some(page_id) = item.page_id.as_deref().filter(|id| !page_id_is_safe(id)) {
                return Ok(ItemOutcome::Rejected(format!(
                    "{} pins {page_id:?}, which is not a usable page id",
                    item.id
                )));
            }
            if let Err(error) = set_pin(drawings_dir, item.page_id.as_deref()) {
                // set_pin records the pin, then promotes the page when it has
                // one. A stored page it cannot read would fail every round, while
                // the pin itself is kept, as a paired host keeps it.
                let page_id = item.page_id.as_deref().unwrap_or_default();
                let stored = crate::pages::page_dir(drawings_dir, page_id)
                    .join(format!("{}.json", crate::pages::PAGE_STEM));
                if item.page_id.is_none()
                    || !stored.exists()
                    || load_page(drawings_dir, page_id).is_ok()
                {
                    return Err(error);
                }
                return Ok(ItemOutcome::Applied(format!(
                    "pinned {page_id}; its stored page cannot be read, so latest.* stays: {error:#}"
                )));
            }
            Ok(ItemOutcome::Applied(match &item.page_id {
                Some(page_id) => format!("pinned {page_id}"),
                None => "cleared the pin".to_owned(),
            }))
        }
        "promote" => {
            let Some(page_id) = &item.page_id else {
                return Ok(ItemOutcome::Rejected(format!(
                    "{} names no page to promote",
                    item.id
                )));
            };
            if let Err(error) = load_page(drawings_dir, page_id) {
                return Ok(ItemOutcome::Rejected(format!(
                    "{} promotes {page_id:?}, which this machine cannot read: {error:#}",
                    item.id
                )));
            }
            promote_page(drawings_dir, page_id)?;
            Ok(ItemOutcome::Applied(format!("promoted {page_id}")))
        }
        other => Ok(ItemOutcome::Rejected(format!(
            "{} has kind {other:?}, which this receiver does not know",
            item.id
        ))),
    }
}

/// Applies items in send order and stops at the first failure that may pass,
/// so nothing after it runs ahead of it. Returns what to confirm and one log
/// line per item handled.
pub fn apply_inbox(items: &[InboxItem], drawings_dir: &Path) -> (Vec<Confirmation>, Vec<String>) {
    let mut confirmations = Vec::new();
    let mut log = Vec::new();
    for item in items {
        match apply_item(item, drawings_dir) {
            Ok(ItemOutcome::Applied(line)) | Ok(ItemOutcome::Rejected(line)) => {
                log.push(line);
                confirmations.push(Confirmation {
                    id: item.id.clone(),
                    version: item.version,
                });
            }
            Err(error) => {
                log.push(format!(
                    "could not apply {}, it comes again on the next fetch: {error:#}",
                    item.id
                ));
                break;
            }
        }
    }
    (confirmations, log)
}

/// Fetches what the hub queued for this machine.
pub fn fetch_inbox(hub_url: &str, timeout: Duration) -> anyhow::Result<Vec<InboxItem>> {
    #[derive(Deserialize)]
    struct Inbox {
        items: Vec<serde_json::Value>,
    }
    let body = run_curl(&format!("{hub_url}/hub/inbox"), timeout, None)?;
    let inbox: Inbox = serde_json::from_slice(&body)
        .map_err(|error| anyhow::anyhow!("the hub's inbox is not the expected JSON: {error}"))?;
    Ok(inbox.items.into_iter().map(read_inbox_item).collect())
}

/// One item that does not parse must not stop the queue (ADR-0010): it becomes
/// an item of an unknown kind, logged and confirmed with the id and version it
/// carries. The hub removes only an exact version match, so an item without a
/// numeric version (only a hub bug makes one) stays queued and is logged again.
fn read_inbox_item(value: serde_json::Value) -> InboxItem {
    let id = value["id"].as_str().unwrap_or_default().to_owned();
    let version = value["version"].as_u64().unwrap_or_default();
    serde_json::from_value(value).unwrap_or_else(|error| InboxItem {
        id,
        version,
        kind: format!("unreadable ({error})"),
        page_id: None,
        snapshot: None,
    })
}

/// Tells the hub what was written; skipped when there is nothing to confirm.
pub fn confirm(
    hub_url: &str,
    timeout: Duration,
    confirmations: &[Confirmation],
) -> anyhow::Result<()> {
    if confirmations.is_empty() {
        return Ok(());
    }
    let body = serde_json::to_vec(&serde_json::json!({ "items": confirmations }))?;
    run_curl(
        &format!("{hub_url}/hub/inbox/confirm"),
        timeout,
        Some(&body),
    )?;
    Ok(())
}

/// HTTPS goes through the system's curl (ADR-0009). A refusal carries the
/// hub's own `{"error": ...}` sentence, which says what to do about it.
fn run_curl(url: &str, timeout: Duration, post_body: Option<&[u8]>) -> anyhow::Result<Vec<u8>> {
    let seconds = timeout.as_secs().max(1).to_string();
    let mut command = Command::new("curl");
    command
        // --disable must come first: it keeps a user's ~/.curlrc out of the parsing.
        // --fail-with-body needs curl 7.76; the status line works on every curl.
        // The hub knows the machine from its tailnet address, so no proxy. A
        // backlog of large sheets may take longer than the timeout, so the timeout
        // applies to connecting and to a transfer that has stalled, not the total.
        .args([
            "--disable",
            "--noproxy",
            "*",
            "--silent",
            "--show-error",
            "--write-out",
            "\n%{http_code}",
        ])
        .args([
            "--connect-timeout",
            &seconds,
            "--speed-limit",
            "1",
            "--speed-time",
            &seconds,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if post_body.is_some() {
        command.args([
            "--request",
            "POST",
            "--header",
            "Content-Type: application/json",
            "--data-binary",
            "@-",
        ]);
    }
    command.arg(url);
    let mut child = command.spawn().map_err(|error| {
        anyhow::anyhow!("could not run curl, which the receiver needs: {error}")
    })?;
    if let (Some(body), Some(mut stdin)) = (post_body, child.stdin.take()) {
        // A curl that exits early closes the pipe; its exit status says why.
        let _ = stdin.write_all(body);
    }
    let output = child.wait_with_output()?;
    let curl_reason = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !output.status.success() {
        anyhow::bail!("could not reach the hub at {url}: {curl_reason}");
    }
    let mut body = output.stdout;
    let status_start = body.iter().rposition(|byte| *byte == b'\n').unwrap_or(0);
    let status = String::from_utf8_lossy(&body[status_start..])
        .trim()
        .to_owned();
    body.truncate(status_start);
    if status.starts_with('2') {
        return Ok(body);
    }
    let hub_reason = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|answer| answer["error"].as_str().map(str::to_owned));
    Err(match hub_reason {
        Some(reason) => anyhow::anyhow!("the hub refused: {reason}"),
        None => anyhow::anyhow!("the hub at {url} answered HTTP {status}"),
    })
}

/// One round: fetch, apply in order, confirm what was handled. What was
/// written is reported before confirming, so a failed confirm cannot hide it.
/// `unconfirmed` carries what was applied but not confirmed into the next
/// round, which confirms it again without writing it again.
pub fn receive_once(
    fetch: impl Fn() -> anyhow::Result<Vec<InboxItem>>,
    confirm: impl Fn(&[Confirmation]) -> anyhow::Result<()>,
    drawings_dir: &Path,
    unconfirmed: &mut Vec<Confirmation>,
    report: &mut impl FnMut(String),
) -> anyhow::Result<()> {
    let items = fetch()?;
    let (already_applied, to_apply): (Vec<InboxItem>, Vec<InboxItem>) =
        items.into_iter().partition(|item| {
            unconfirmed.contains(&Confirmation {
                id: item.id.clone(),
                version: item.version,
            })
        });
    let (applied, log) = apply_inbox(&to_apply, drawings_dir);
    log.into_iter().for_each(&mut *report);
    *unconfirmed = already_applied
        .into_iter()
        .map(|item| Confirmation {
            id: item.id,
            version: item.version,
        })
        .chain(applied)
        .collect();
    confirm(unconfirmed)?;
    unconfirmed.clear();
    Ok(())
}

/// Runs until the process is stopped. Every problem is logged and retried
/// after the interval; nothing here ends the loop.
pub fn run_receiver(
    hub_url: &str,
    drawings_dir: &Path,
    interval: Duration,
    timeout: Duration,
) -> ! {
    let hub_url = hub_url.trim_end_matches('/');
    println!(
        "Receiving sketches from {hub_url} into {} every {} s",
        drawings_dir.display(),
        interval.as_secs()
    );
    let mut last_error = String::new();
    let mut unconfirmed = Vec::new();
    loop {
        match receive_once(
            || fetch_inbox(hub_url, timeout),
            |confirmations| confirm(hub_url, timeout, confirmations),
            drawings_dir,
            &mut unconfirmed,
            &mut |line| println!("{line}"),
        ) {
            Ok(()) => last_error.clear(),
            Err(error) => {
                // A hub that keeps refusing would fill the journal once per interval.
                let message = format!("{error:#}");
                if message != last_error {
                    eprintln!("{message}");
                    last_error = message;
                }
            }
        }
        std::thread::sleep(interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn sheet_item(page_id: &str, version: u64) -> InboxItem {
        InboxItem {
            id: format!("sheet-{page_id}"),
            version,
            kind: "sheet".to_owned(),
            page_id: Some(page_id.to_owned()),
            snapshot: Some(serde_json::json!({
                "schemaVersion": 2,
                "page": {"id": page_id},
                "canvas": {"width": 32, "height": 24, "background": "#ffffff"},
                "strokes": []
            })),
        }
    }

    fn stamp_item(kind: &str, page_id: Option<&str>, version: u64) -> InboxItem {
        InboxItem {
            id: format!("stamp-{version}"),
            version,
            kind: kind.to_owned(),
            page_id: page_id.map(str::to_owned),
            snapshot: None,
        }
    }

    fn latest_updated_at(drawings_dir: &Path) -> u64 {
        let latest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(drawings_dir.join("latest.json")).unwrap(),
        )
        .unwrap();
        latest["updatedAt"].as_u64().unwrap()
    }

    fn fresh_drawings_dir() -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let drawings_dir = root.path().join("not-yet").join("drawings");
        (root, drawings_dir)
    }

    #[test]
    fn a_sheet_is_written_as_latest() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        let (confirmations, log) = apply_inbox(&[sheet_item("page-a", 7)], &drawings_dir);
        assert!(drawings_dir.join("latest.json").exists(), "log: {log:?}");
        assert_eq!(
            confirmations,
            vec![Confirmation {
                id: "sheet-page-a".to_owned(),
                version: 7
            }]
        );
    }

    #[test]
    fn a_narrated_sheet_writes_the_timeline_the_narrated_skill_reads() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        let mut item = sheet_item("talk", 1);
        item.snapshot = Some(serde_json::json!({
            "schemaVersion": crate::drawing::NARRATED_SCHEMA_VERSION,
            "page": {"id": "talk", "title": "Talk"},
            "canvas": {"width": 200, "height": 200, "background": "#ffffff"},
            "strokes": [{
                "id": "stroke-1",
                "color": "#111827",
                "width": 4,
                "startedAt": 1500,
                "points": [
                    {"x": 20, "y": 20, "pressure": 0.5, "t": 0},
                    {"x": 60, "y": 60, "pressure": 0.5, "t": 100}
                ]
            }],
            "narration": {
                "language": "nl",
                "segments": [{"start": 1000, "end": 3000, "text": "Eerst de database."}]
            }
        }));

        let (_, log) = apply_inbox(&[item], &drawings_dir);

        let timeline = std::fs::read_to_string(drawings_dir.join("latest.timeline.md"))
            .unwrap_or_else(|error| panic!("no timeline ({error}), log: {log:?}"));
        assert!(timeline.contains("> Eerst de database."));
        assert!(drawings_dir.join("latest.steps").join("001.png").exists());

        apply_inbox(&[sheet_item("plain", 2)], &drawings_dir);
        assert!(!drawings_dir.join("latest.timeline.md").exists());
        assert!(!drawings_dir.join("latest.steps").exists());
    }

    #[test]
    fn a_pin_after_its_sheet_in_one_fetch_points_latest_at_that_page() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        let items = [sheet_item("page-a", 1)];
        apply_inbox(&items, &drawings_dir);
        let page_a_updated_at = latest_updated_at(&drawings_dir);
        std::thread::sleep(std::time::Duration::from_millis(5));

        let items = [
            sheet_item("page-a", 2),
            sheet_item("page-b", 3),
            stamp_item("pin", Some("page-a"), 4),
        ];
        let (confirmations, log) = apply_inbox(&items, &drawings_dir);
        assert_eq!(confirmations.len(), 3, "log: {log:?}");
        assert_eq!(
            crate::pages::read_pin(&drawings_dir).as_deref(),
            Some("page-a")
        );
        let latest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(drawings_dir.join("latest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(latest["page"]["id"], "page-a");
        assert!(latest_updated_at(&drawings_dir) >= page_a_updated_at);
    }

    #[test]
    fn an_invalid_sheet_is_confirmed_and_logged() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        let mut item = sheet_item("page-a", 9);
        item.snapshot.as_mut().unwrap()["schemaVersion"] = serde_json::json!(99);
        let (confirmations, log) = apply_inbox(&[item], &drawings_dir);
        assert_eq!(confirmations.len(), 1);
        assert!(
            log.iter()
                .any(|line| line.contains("unsupported schemaVersion")),
            "log: {log:?}"
        );
        assert!(!drawings_dir.join("latest.json").exists());
    }

    #[test]
    fn an_unknown_kind_is_confirmed_and_logged() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        let (confirmations, log) = apply_inbox(&[stamp_item("rename", None, 3)], &drawings_dir);
        assert_eq!(confirmations.len(), 1);
        assert!(
            log.iter().any(|line| line.contains("rename")),
            "log: {log:?}"
        );
    }

    #[test]
    fn a_promote_of_a_page_this_machine_does_not_have_is_confirmed_and_logged() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        let (confirmations, log) = apply_inbox(
            &[stamp_item("promote", Some("never-sent"), 3)],
            &drawings_dir,
        );
        assert_eq!(confirmations.len(), 1, "log: {log:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_write_failure_stops_before_later_items() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let drawings_dir = root.path().join("drawings");
        std::fs::create_dir_all(&drawings_dir).unwrap();
        std::fs::set_permissions(&drawings_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        if std::fs::write(drawings_dir.join("probe"), "").is_ok() {
            eprintln!("skipped: a read-only folder is writable here (running as root)");
            return;
        }

        let items = [
            sheet_item("page-a", 1),
            stamp_item("pin", Some("page-a"), 2),
        ];
        let (confirmations, log) = apply_inbox(&items, &drawings_dir);

        std::fs::set_permissions(&drawings_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(confirmations.is_empty(), "confirmed {confirmations:?}");
        assert_eq!(log.len(), 1, "log: {log:?}");
    }

    #[test]
    fn after_a_failed_confirm_the_next_round_confirms_without_writing_again() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        let mut unconfirmed = Vec::new();
        let first = receive_once(
            || Ok(vec![sheet_item("page-a", 4)]),
            |_: &[Confirmation]| Err(anyhow::anyhow!("the confirm timed out")),
            &drawings_dir,
            &mut unconfirmed,
            &mut |_| {},
        );
        assert!(first.is_err());
        let written_at = latest_updated_at(&drawings_dir);
        let confirmed = std::cell::RefCell::new(Vec::new());
        let mut log = Vec::new();
        receive_once(
            || Ok(vec![sheet_item("page-a", 4)]),
            |confirmations: &[Confirmation]| {
                confirmed.borrow_mut().extend_from_slice(confirmations);
                Ok(())
            },
            &drawings_dir,
            &mut unconfirmed,
            &mut |line| log.push(line),
        )
        .unwrap();
        assert!(log.is_empty(), "written again: {log:?}");
        assert_eq!(latest_updated_at(&drawings_dir), written_at);
        assert_eq!(
            *confirmed.borrow(),
            vec![Confirmation {
                id: "sheet-page-a".to_owned(),
                version: 4
            }]
        );
        assert!(unconfirmed.is_empty());
    }

    #[test]
    fn receive_once_confirms_what_it_applied() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        let confirmed = std::cell::RefCell::new(Vec::new());
        let mut log = Vec::new();
        receive_once(
            || Ok(vec![sheet_item("page-a", 4)]),
            |confirmations: &[Confirmation]| {
                confirmed.borrow_mut().extend(
                    confirmations
                        .iter()
                        .map(|confirmation| (confirmation.id.clone(), confirmation.version)),
                );
                Ok(())
            },
            &drawings_dir,
            &mut Vec::new(),
            &mut |line| log.push(line),
        )
        .unwrap();
        assert_eq!(*confirmed.borrow(), vec![("sheet-page-a".to_owned(), 4)]);
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn receive_once_reports_a_fetch_error_and_confirms_nothing() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        let confirm_called = std::cell::Cell::new(false);
        let result = receive_once(
            || {
                Err(anyhow::anyhow!(
                    "paperclip is not allowed on the sketch hub"
                ))
            },
            |_: &[Confirmation]| {
                confirm_called.set(true);
                Ok(())
            },
            &drawings_dir,
            &mut Vec::new(),
            &mut |_| {},
        );
        let error = result.expect_err("a fetch error is reported").to_string();
        assert!(error.contains("not allowed"), "{error}");
        assert!(!confirm_called.get());
    }

    #[test]
    fn receive_once_reports_what_it_wrote_even_when_confirm_fails() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        let mut log = Vec::new();
        let result = receive_once(
            || Ok(vec![sheet_item("page-a", 4)]),
            |_: &[Confirmation]| Err(anyhow::anyhow!("the confirm timed out")),
            &drawings_dir,
            &mut Vec::new(),
            &mut |line| log.push(line),
        );
        assert!(result.is_err());
        assert_eq!(log, vec!["wrote sheet-page-a".to_owned()]);
    }

    #[test]
    fn a_pin_with_an_unusable_page_id_is_confirmed_and_logged() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        let (confirmations, log) = apply_inbox(
            &[stamp_item("pin", Some("../x"), 1), sheet_item("page-b", 2)],
            &drawings_dir,
        );
        assert_eq!(confirmations.len(), 2, "{log:?}");
        assert!(drawings_dir.join("latest.json").exists());
    }

    #[test]
    fn a_pin_of_a_page_this_machine_cannot_read_is_kept_and_confirmed() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        let page = crate::pages::page_dir(&drawings_dir, "broken");
        std::fs::create_dir_all(&page).unwrap();
        std::fs::write(page.join("page.json"), "{ half written").unwrap();
        let (confirmations, log) = apply_inbox(
            &[
                stamp_item("pin", Some("broken"), 1),
                sheet_item("page-b", 2),
            ],
            &drawings_dir,
        );
        assert_eq!(confirmations.len(), 2, "{log:?}");
        let pinned =
            std::fs::read_to_string(crate::pages::pages_dir(&drawings_dir).join("pinned.json"))
                .unwrap();
        assert!(pinned.contains("broken"), "the pin is kept: {pinned}");
    }

    #[test]
    fn a_promote_of_a_page_folder_without_its_page_file_is_confirmed_and_logged() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        std::fs::create_dir_all(crate::pages::page_dir(&drawings_dir, "half-written")).unwrap();
        let (confirmations, log) = apply_inbox(
            &[stamp_item("promote", Some("half-written"), 1)],
            &drawings_dir,
        );
        assert_eq!(confirmations.len(), 1, "{log:?}");
    }

    #[test]
    fn fetch_inbox_sets_a_malformed_item_aside_and_keeps_the_rest() {
        if !curl_is_installed() {
            eprintln!("skipped: curl is not installed");
            return;
        }
        let (url, hub) = one_shot_hub(
            "200 OK",
            r#"{"items":[{"id":"stamp-1","sequence":1,"version":null,"kind":"pin"},{"id":"stamp-2","sequence":2,"version":2,"kind":"pin","pageId":null}]}"#,
        );
        let items = fetch_inbox(&url, std::time::Duration::from_secs(5)).unwrap();
        hub.join().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "stamp-1");
        assert_eq!(items[1].kind, "pin");
        let (_root, drawings_dir) = fresh_drawings_dir();
        let (confirmations, log) = apply_inbox(&items[..1], &drawings_dir);
        assert_eq!(
            confirmations.len(),
            1,
            "a malformed item is confirmed: {log:?}"
        );
    }

    /// Answers one HTTP request with the given status and body, and hands back
    /// the request it received.
    fn one_shot_hub(status_line: &str, body: &str) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let response = format!(
            "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_millis(500)))
                .unwrap();
            let mut received = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        received.extend_from_slice(&buffer[..count]);
                        let text = String::from_utf8_lossy(&received);
                        if let Some(header_end) = text.find("\r\n\r\n") {
                            let length = text[..header_end]
                                .lines()
                                .find_map(|line| {
                                    line.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .map(|value| value.trim().parse::<usize>().unwrap_or(0))
                                })
                                .unwrap_or(0);
                            if received.len() >= header_end + 4 + length {
                                break;
                            }
                        }
                    }
                }
            }
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&received).into_owned()
        });
        (url, handle)
    }

    fn curl_is_installed() -> bool {
        std::process::Command::new("curl")
            .arg("--version")
            .output()
            .is_ok()
    }

    #[test]
    fn fetch_inbox_reads_items_from_a_hub() {
        if !curl_is_installed() {
            eprintln!("skipped: curl is not installed");
            return;
        }
        let (url, hub) = one_shot_hub(
            "200 OK",
            r#"{"items":[{"id":"stamp-3","sequence":3,"version":3,"kind":"pin","pageId":null}]}"#,
        );
        let items = fetch_inbox(&url, std::time::Duration::from_secs(5)).unwrap();
        let request = hub.join().unwrap();
        assert!(request.starts_with("GET /hub/inbox "), "{request}");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "stamp-3");
        assert_eq!(items[0].page_id, None);
    }

    #[test]
    fn fetch_inbox_carries_the_hubs_reason_on_403() {
        if !curl_is_installed() {
            eprintln!("skipped: curl is not installed");
            return;
        }
        let (url, hub) = one_shot_hub(
            "403 Forbidden",
            r#"{"error":"paperclip is not allowed on the sketch hub; allow it on the dev-portal page"}"#,
        );
        let error = fetch_inbox(&url, std::time::Duration::from_secs(5))
            .expect_err("a 403 is an error")
            .to_string();
        hub.join().unwrap();
        assert!(
            error.contains("paperclip is not allowed on the sketch hub"),
            "{error}"
        );
    }

    #[test]
    fn confirm_posts_the_items_with_their_versions() {
        if !curl_is_installed() {
            eprintln!("skipped: curl is not installed");
            return;
        }
        let (url, hub) = one_shot_hub("200 OK", r#"{"removed":1}"#);
        confirm(
            &url,
            std::time::Duration::from_secs(5),
            &[Confirmation {
                id: "sheet-page-a".to_owned(),
                version: 12,
            }],
        )
        .unwrap();
        let request = hub.join().unwrap();
        assert!(request.starts_with("POST /hub/inbox/confirm "), "{request}");
        assert!(
            request.contains(r#"{"items":[{"id":"sheet-page-a","version":12}]}"#),
            "{request}"
        );
    }
}
