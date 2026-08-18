//! Putting a saved sheet on the system clipboard from the command line, so a
//! hotkey can paste the drawing into a chat window without the app being open.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::pages::{page_dir, page_id_is_safe, PAGE_STEM};

const LATEST_STEM: &str = "latest";

pub struct SheetImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// The PNG a copy reads: `latest.png` by default, or one page's own copy.
///
/// The page id arrives from the command line and becomes a directory name, so
/// it goes through the same alphabet check the writer uses before it is joined
/// to anything.
pub fn sheet_image_path(drawings_dir: &Path, page_id: Option<&str>) -> anyhow::Result<PathBuf> {
    let Some(page_id) = page_id else {
        return Ok(drawings_dir.join(format!("{LATEST_STEM}.png")));
    };
    if !page_id_is_safe(page_id) {
        anyhow::bail!("Unusable page id; use an id listed in the pages index");
    }
    Ok(page_dir(drawings_dir, page_id).join(format!("{PAGE_STEM}.png")))
}

/// A sheet's stamp lives beside its image under the same stem, for both
/// `latest.*` and a page's own files.
pub fn sheet_json_path(image_path: &Path) -> PathBuf {
    image_path.with_extension("json")
}

/// When the sheet was last written, in unix milliseconds. A missing or
/// unreadable stamp is `None` rather than an error: the image is what was asked
/// for, and its age is the extra.
pub fn read_updated_at(json_path: &Path) -> Option<u128> {
    let text = std::fs::read_to_string(json_path).ok()?;
    let stored: serde_json::Value = serde_json::from_str(&text).ok()?;
    stored["updatedAt"].as_u64().map(u128::from)
}

/// How old the sheet is, in words. Relative rather than a wall clock reading,
/// because the question a copy answers is whether this is the sheet just drawn.
///
/// A stamp from the future means two clocks disagree, not that a sheet is
/// negatively old, so it reads as just now.
pub fn describe_age(updated_at_millis: u128, now_millis: u128) -> String {
    let seconds = now_millis.saturating_sub(updated_at_millis) / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;

    if seconds < 60 {
        "just now".to_owned()
    } else if minutes < 60 {
        ago(minutes, "minute")
    } else if hours < 24 {
        ago(hours, "hour")
    } else {
        ago(hours / 24, "day")
    }
}

fn ago(count: u128, unit: &str) -> String {
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

/// Decodes the sheet into the raw RGBA the clipboard wants.
pub fn read_sheet_image(image_path: &Path) -> anyhow::Result<SheetImage> {
    let png_bytes = std::fs::read(image_path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => anyhow::anyhow!(
            "No sheet at {}; draw one and save it, then copy again.",
            image_path.display()
        ),
        _ => anyhow::anyhow!("Could not read {}: {error}", image_path.display()),
    })?;

    let decoded = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png)
        .map_err(|error| anyhow::anyhow!("Could not read {}: {error}", image_path.display()))?
        .into_rgba8();
    let width = decoded.width() as usize;
    let height = decoded.height() as usize;

    Ok(SheetImage {
        width,
        height,
        rgba: decoded.into_raw(),
    })
}

/// The one step no test can stand in for, since it needs a running desktop
/// session. Everything it depends on above is a file read.
///
/// ponytail: on macOS the pasteboard is a system service, so the image outlives
/// this process. An X11 session hands the clipboard back when its owner exits,
/// which would need a copy command that stays running instead.
#[allow(dead_code)] // Called from main.rs; the test include compiles this module alone.
pub fn copy_image_to_clipboard(image: &SheetImage) -> anyhow::Result<()> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|_| anyhow::anyhow!("Clipboard unavailable; nothing was copied"))?;
    clipboard
        .set_image(arboard::ImageData {
            width: image.width,
            height: image.height,
            bytes: Cow::from(image.rgba.as_slice()),
        })
        .map_err(|_| anyhow::anyhow!("Clipboard unavailable; nothing was copied"))
}
