//! The receiving end of the dev-portal sketch hub: fetch what the hub queued
//! for this machine, write it the way a host writes a direct upload, confirm it.
//! The contract lives in dev-portal's `docs/sketch-hub-api.md`.

use std::path::Path;

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
}
