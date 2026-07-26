use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::drawing::{CanvasSize, DrawingSnapshot, PageRef, Stroke, LEGACY_PAGE_ID};
use crate::export::{write_artifacts, write_snapshot, ExportedFiles};

const PAGES_DIRECTORY: &str = "pages";
const PAGE_STEM: &str = "page";
const INDEX_FILE: &str = "index.json";
const PINNED_FILE: &str = "pinned.json";
const MAX_PAGE_ID_LENGTH: usize = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageEntry {
    #[serde(rename = "pageId")]
    pub page_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: u128,
    /// When the page directory first appeared. Taken from the filesystem rather
    /// than the snapshot, so sheet numbers stay put instead of reshuffling every
    /// time some other sheet is edited.
    #[serde(rename = "createdAt")]
    pub created_at: u128,
    #[serde(rename = "strokeCount")]
    pub stroke_count: usize,
    pub files: PageEntryFiles,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageEntryFiles {
    pub json: String,
    pub svg: String,
    pub png: String,
}

#[derive(Serialize, Deserialize)]
struct PinnedPage {
    #[serde(rename = "pageId")]
    page_id: Option<String>,
}

#[derive(Serialize)]
struct PageIndex<'a> {
    #[serde(rename = "updatedAt")]
    updated_at: u128,
    /// The page `latest.*` follows, so a reader of the index alone can tell
    /// which sheet the agent is looking at.
    #[serde(rename = "pinnedPageId", skip_serializing_if = "Option::is_none")]
    pinned_page_id: Option<String>,
    pages: &'a [PageEntry],
}

/// A written `page.json`: the snapshot plus the `updatedAt` stamp the exporter
/// adds. Used to rebuild the index and to reload a page for promotion.
///
/// The snapshot fields are repeated rather than `#[serde(flatten)]`ed: flatten
/// buffers through serde's internal content type, which cannot carry the `u128`
/// timestamps on every point, so flattening silently fails to parse every page.
#[derive(Deserialize)]
struct StoredPage {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    #[serde(default)]
    page: Option<PageRef>,
    canvas: CanvasSize,
    strokes: Vec<Stroke>,
    #[serde(rename = "updatedAt", default)]
    updated_at: u128,
}

impl StoredPage {
    fn into_snapshot(self) -> DrawingSnapshot {
        DrawingSnapshot {
            schema_version: self.schema_version,
            page: self.page,
            canvas: self.canvas,
            strokes: self.strokes,
        }
    }
}

/// The page id becomes a directory name, so it is a trust boundary: anything
/// outside this alphabet could escape the drawings directory.
pub fn page_id_is_safe(page_id: &str) -> bool {
    !page_id.is_empty()
        && page_id.len() <= MAX_PAGE_ID_LENGTH
        && page_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// The page a snapshot belongs to. Schema version 1 clients have no page, so
/// their work is filed under a reserved id rather than being refused.
pub fn page_id_for(snapshot: &DrawingSnapshot) -> Option<String> {
    match snapshot.page.as_ref() {
        Some(page) => page_id_is_safe(&page.id).then(|| page.id.clone()),
        None => Some(LEGACY_PAGE_ID.to_owned()),
    }
}

pub fn pages_dir(drawings_dir: &Path) -> PathBuf {
    drawings_dir.join(PAGES_DIRECTORY)
}

pub fn page_dir(drawings_dir: &Path, page_id: &str) -> PathBuf {
    pages_dir(drawings_dir).join(page_id)
}

/// Writes the page's own copy, then refreshes the index. `latest.*` is mirrored
/// from this page only when the user has not pinned a different one — pinning is
/// how the agent's view stops drifting to whatever was drawn on last.
///
/// With nothing pinned this stays the original last-write-wins behaviour, so
/// consumers like the installed `/goghmode` skill keep working untouched.
pub fn write_page(
    snapshot: &DrawingSnapshot,
    drawings_dir: impl AsRef<Path>,
) -> anyhow::Result<ExportedFiles> {
    let drawings_dir = drawings_dir.as_ref();
    let page_id = page_id_for(snapshot)
        .ok_or_else(|| anyhow::anyhow!("Snapshot carries an unusable page id"))?;

    let page_files = write_artifacts(
        snapshot,
        page_dir(drawings_dir, &page_id),
        PAGE_STEM,
        &format!("drawings/{PAGES_DIRECTORY}/{page_id}/"),
        None,
    )?;
    rebuild_index(drawings_dir)?;

    let pinned = read_pin(drawings_dir);
    if pinned.is_some_and(|pinned| pinned != page_id) {
        return Ok(page_files);
    }

    write_artifacts(
        snapshot,
        drawings_dir,
        "latest",
        "drawings/",
        Some(page_files.updated_at),
    )
}

/// The page `latest.*` follows, or `None` when it follows whatever was written
/// last.
pub fn read_pin(drawings_dir: impl AsRef<Path>) -> Option<String> {
    let path = pages_dir(drawings_dir.as_ref()).join(PINNED_FILE);
    let stored: PinnedPage = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    let page_id = stored.page_id?;
    page_id_is_safe(&page_id).then_some(page_id)
}

/// Pins a page, or clears the pin with `None`. Pinning immediately points
/// `latest.*` at that page, so the control does what it says at the moment it is
/// used rather than at the next save.
pub fn set_pin(drawings_dir: impl AsRef<Path>, page_id: Option<&str>) -> anyhow::Result<()> {
    let drawings_dir = drawings_dir.as_ref();
    if let Some(page_id) = page_id {
        if !page_id_is_safe(page_id) {
            anyhow::bail!("Unusable page id");
        }
    }

    let directory = pages_dir(drawings_dir);
    fs::create_dir_all(&directory)?;
    let path = directory.join(PINNED_FILE);
    let tmp = directory.join(format!("{PINNED_FILE}.tmp"));
    let stored = PinnedPage {
        page_id: page_id.map(str::to_owned),
    };
    fs::write(&tmp, serde_json::to_string_pretty(&stored)?)?;
    fs::rename(&tmp, &path)?;
    // The index carries the pin, so it is stale the moment the pin moves.
    rebuild_index(drawings_dir)?;

    // A pin declares which page `latest.*` follows, so it may name a page this Mac
    // has not received yet — a sheet created on the iPad and stamped before anything
    // was drawn on it. `write_page` honours the pin the moment that page arrives.
    // Refusing here made stamping such a sheet fail with a 400.
    if let Some(page_id) = page_id {
        if page_dir(drawings_dir, page_id)
            .join(format!("{PAGE_STEM}.json"))
            .exists()
        {
            promote_page(drawings_dir, page_id)?;
        }
    }
    Ok(())
}

/// Points `latest.*` at a stored page without touching the pin — the "send this
/// one now" action.
pub fn promote_page(drawings_dir: impl AsRef<Path>, page_id: &str) -> anyhow::Result<ExportedFiles> {
    let drawings_dir = drawings_dir.as_ref();
    let snapshot = load_page_snapshot(drawings_dir, page_id)?;
    write_snapshot(&snapshot, drawings_dir)
}

/// Rebuilt from the directory rather than maintained incrementally: no drift,
/// no repair path, and a future `pages/.trash/<id>/page.json` sits one level
/// deeper so it drops out without needing a filter.
/// ponytail: O(pages) rescan per save, fine to a few hundred pages.
pub fn rebuild_index(drawings_dir: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
    let drawings_dir = drawings_dir.as_ref();
    let pages = list_pages(drawings_dir);
    let updated_at = pages.first().map(|page| page.updated_at).unwrap_or(0);

    let directory = pages_dir(drawings_dir);
    fs::create_dir_all(&directory)?;
    let index_path = directory.join(INDEX_FILE);
    let index_tmp = directory.join(format!("{INDEX_FILE}.tmp"));
    let index = PageIndex {
        updated_at,
        pinned_page_id: read_pin(drawings_dir),
        pages: &pages,
    };
    fs::write(&index_tmp, serde_json::to_string_pretty(&index)?)?;
    fs::rename(&index_tmp, &index_path)?;

    Ok(index_path)
}

/// Newest first, which is the order the Mac browser shows them in.
pub fn list_pages(drawings_dir: impl AsRef<Path>) -> Vec<PageEntry> {
    let directory = pages_dir(drawings_dir.as_ref());
    let Ok(entries) = fs::read_dir(&directory) else {
        return Vec::new();
    };

    let mut pages: Vec<PageEntry> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let page_id = entry.file_name().to_string_lossy().into_owned();
            if !page_id_is_safe(&page_id) {
                return None;
            }
            let stored = read_stored_page(&entry.path().join(format!("{PAGE_STEM}.json")))?;
            Some(PageEntry {
                title: stored.page.as_ref().and_then(|page| page.title.clone()),
                updated_at: stored.updated_at,
                created_at: directory_created_at(&entry.path()).unwrap_or(stored.updated_at),
                stroke_count: stored.strokes.len(),
                files: PageEntryFiles {
                    json: format!("drawings/{PAGES_DIRECTORY}/{page_id}/{PAGE_STEM}.json"),
                    svg: format!("drawings/{PAGES_DIRECTORY}/{page_id}/{PAGE_STEM}.svg"),
                    png: format!("drawings/{PAGES_DIRECTORY}/{page_id}/{PAGE_STEM}.png"),
                },
                page_id,
            })
        })
        .collect();

    pages.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.page_id.cmp(&right.page_id))
    });
    pages
}

/// Reloads a stored page so it can be promoted back to `latest.*`.
pub fn load_page_snapshot(
    drawings_dir: impl AsRef<Path>,
    page_id: &str,
) -> anyhow::Result<DrawingSnapshot> {
    if !page_id_is_safe(page_id) {
        anyhow::bail!("Unusable page id");
    }
    let path = page_dir(drawings_dir.as_ref(), page_id).join(format!("{PAGE_STEM}.json"));
    read_stored_page(&path)
        .map(StoredPage::into_snapshot)
        .ok_or_else(|| anyhow::anyhow!("Could not read {}", path.display()))
}

/// Creation time of a page directory, in unix milliseconds. Not every platform
/// records one; callers fall back to the last write.
fn directory_created_at(path: &Path) -> Option<u128> {
    fs::metadata(path)
        .and_then(|metadata| metadata.created())
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|since_epoch| since_epoch.as_millis())
}

/// Sheet numbers in creation order, so a number belongs to a sheet for as long
/// as the sheet exists.
pub fn sheet_numbers(pages: &[PageEntry]) -> std::collections::HashMap<String, usize> {
    let mut ordered: Vec<&PageEntry> = pages.iter().collect();
    ordered.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.page_id.cmp(&right.page_id))
    });
    ordered
        .into_iter()
        .enumerate()
        .map(|(index, page)| (page.page_id.clone(), index + 1))
        .collect()
}

fn read_stored_page(path: &Path) -> Option<StoredPage> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}
