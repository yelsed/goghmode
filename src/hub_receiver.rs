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
use crate::pages::{promote_page, set_pin, write_page};

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
#[derive(Debug, PartialEq, Serialize)]
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
            let Some(raw) = item.snapshot.clone() else {
                return Ok(ItemOutcome::Rejected(format!(
                    "{} carries no snapshot",
                    item.id
                )));
            };
            let snapshot: DrawingSnapshot = match serde_json::from_value(raw) {
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
            set_pin(drawings_dir, item.page_id.as_deref())?;
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
            if !crate::pages::page_dir(drawings_dir, page_id).exists() {
                return Ok(ItemOutcome::Rejected(format!(
                    "{} promotes {page_id}, which this machine does not have",
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
        items: Vec<InboxItem>,
    }
    let body = run_curl(&format!("{hub_url}/hub/inbox"), timeout, None)?;
    let inbox: Inbox = serde_json::from_slice(&body)
        .map_err(|error| anyhow::anyhow!("the hub's inbox is not the expected JSON: {error}"))?;
    Ok(inbox.items)
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
    let mut command = Command::new("curl");
    command
        .args(["--silent", "--show-error", "--fail-with-body", "--max-time"])
        .arg(timeout.as_secs().max(1).to_string())
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
    if let Some(body) = post_body {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(body)?;
        }
    } else {
        drop(child.stdin.take());
    }
    let output = child.wait_with_output()?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let hub_reason = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .ok()
        .and_then(|answer| answer["error"].as_str().map(str::to_owned));
    let curl_reason = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(match hub_reason {
        Some(reason) => anyhow::anyhow!("the hub refused: {reason}"),
        None => anyhow::anyhow!("could not reach the hub at {url}: {curl_reason}"),
    })
}

/// One round: fetch, apply in order, confirm what was handled.
pub fn receive_once(
    fetch: impl Fn() -> anyhow::Result<Vec<InboxItem>>,
    confirm: impl Fn(&[Confirmation]) -> anyhow::Result<()>,
    drawings_dir: &Path,
) -> anyhow::Result<Vec<String>> {
    let items = fetch()?;
    let (confirmations, log) = apply_inbox(&items, drawings_dir);
    confirm(&confirmations)?;
    Ok(log)
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
    loop {
        match receive_once(
            || fetch_inbox(hub_url, timeout),
            |confirmations| confirm(hub_url, timeout, confirmations),
            drawings_dir,
        ) {
            Ok(log) => {
                last_error.clear();
                for line in log {
                    println!("{line}");
                }
            }
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
    fn receive_once_confirms_what_it_applied() {
        let (_root, drawings_dir) = fresh_drawings_dir();
        let confirmed = std::cell::RefCell::new(Vec::new());
        let log = receive_once(
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
        );
        let error = result.err().expect("a fetch error is reported").to_string();
        assert!(error.contains("not allowed"), "{error}");
        assert!(!confirm_called.get());
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
            .err()
            .expect("a 403 is an error")
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
