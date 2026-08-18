//! The Raycast script command, installed as a file Raycast can find.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const SCRIPT_FILE_NAME: &str = "copy-latest-sheet.sh";

/// One copy of the script text. The file in the repository is the source and
/// the binary carries the same bytes, so an edit cannot land in only one of them.
pub const COPY_LATEST_SHEET_SCRIPT: &str = include_str!("../raycast/copy-latest-sheet.sh");

/// Raycast is pointed at whole directories rather than single files, so the
/// script gets one of its own instead of sharing the support directory with the
/// installed binary.
pub fn default_script_dir(home_dir: &Path) -> PathBuf {
    home_dir
        .join("Library")
        .join("Application Support")
        .join("GoghMode")
        .join("raycast")
}

#[allow(dead_code)] // Called from main.rs; the test include compiles this module alone.
pub fn install_raycast_script(directory: &Path) -> anyhow::Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let path = directory.join(SCRIPT_FILE_NAME);
    fs::write(&path, COPY_LATEST_SHEET_SCRIPT)?;
    // Raycast runs the file itself, so a script without the executable bit is
    // one Raycast lists and then refuses to run.
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    Ok(path)
}
