use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Installs the host so the desktop environment can launch it. Each platform
/// keeps its own idea of what "installed" means — an application bundle on
/// macOS, a desktop entry on Linux — and this is the only place that has to
/// know which one is in play.
#[allow(dead_code)] // Called from main.rs; the test include compiles this module alone.
pub fn install_app(home_dir: &Path, executable_path: &Path) -> anyhow::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        install_macos_app(home_dir, executable_path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        linux::install(home_dir, executable_path)
    }
}

pub fn app_bundle_path(home_dir: &Path) -> PathBuf {
    home_dir.join("Applications").join("GoghMode.app")
}

/// A macOS build compiles all of this and calls none of it, and the tests
/// exercise it on whichever platform they run on. One allow covers the lot
/// rather than one per item.
#[allow(dead_code)]
pub mod linux {
    use super::*;

    const ICON_SVG: &[u8] = include_bytes!("../mobile/icon.svg");

    pub fn desktop_entry_path(home_dir: &Path) -> PathBuf {
        home_dir
            .join(".local")
            .join("share")
            .join("applications")
            .join("goghmode.desktop")
    }

    pub fn desktop_icon_path(home_dir: &Path) -> PathBuf {
        home_dir
            .join(".local")
            .join("share")
            .join("icons")
            .join("hicolor")
            .join("scalable")
            .join("apps")
            .join("goghmode.svg")
    }

    /// Writes a user-level desktop entry and its icon. Everything it touches is
    /// under `~/.local/share`, which Omarchy does not manage — nothing here
    /// reads or writes `~/.local/share/omarchy/`.
    ///
    /// The binary is not copied. On Linux the user built it and chose where it
    /// lives, so the entry points at it where it already is.
    /// ponytail: absolute path in `Exec`, so moving the binary means running
    /// `install-app` again. Copying it in would need its own update story.
    pub fn install(home_dir: &Path, executable_path: &Path) -> anyhow::Result<PathBuf> {
        let entry_path = desktop_entry_path(home_dir);
        let icon_path = desktop_icon_path(home_dir);
        for directory in [entry_path.parent(), icon_path.parent()]
            .into_iter()
            .flatten()
        {
            fs::create_dir_all(directory)?;
        }

        fs::write(&icon_path, ICON_SVG)?;
        fs::write(&entry_path, desktop_entry(executable_path))?;

        Ok(entry_path)
    }

    /// `Exec` is quoted because an unquoted path containing a space reads as an
    /// executable followed by an argument.
    fn desktop_entry(executable_path: &Path) -> String {
        format!(
            r#"[Desktop Entry]
Type=Application
Name=GoghMode
Comment=Local sketchpad for terminal AI workflows
Exec="{}"
Icon=goghmode
Terminal=false
Categories=Graphics;Utility;
StartupWMClass=GoghMode
"#,
            executable_path.display()
        )
    }
}

pub fn install_macos_app(home_dir: &Path, executable_path: &Path) -> anyhow::Result<PathBuf> {
    let bundle = app_bundle_path(home_dir);
    let contents_dir = bundle.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    fs::create_dir_all(&macos_dir)?;

    let support_dir = home_dir
        .join("Library")
        .join("Application Support")
        .join("GoghMode");
    fs::create_dir_all(&support_dir)?;
    let support_binary = support_dir.join("goghmode-bin");
    fs::copy(executable_path, &support_binary)?;
    fs::set_permissions(&support_binary, fs::Permissions::from_mode(0o755))?;
    codesign_macos_binary_if_needed(&support_binary)?;
    let obsolete_bundled_binary = macos_dir.join("goghmode-bin");
    if obsolete_bundled_binary.exists() {
        fs::remove_file(obsolete_bundled_binary)?;
    }

    let launcher = macos_dir.join("GoghMode");
    fs::write(&launcher, launcher_script())?;
    fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755))?;

    fs::write(contents_dir.join("Info.plist"), info_plist())?;

    Ok(bundle)
}

pub(crate) fn is_macho_executable(path: &Path) -> anyhow::Result<bool> {
    let mut file = fs::File::open(path)?;
    let mut magic = [0_u8; 4];
    let bytes_read = file.read(&mut magic)?;
    if bytes_read < magic.len() {
        return Ok(false);
    }

    Ok(matches!(
        magic,
        [0xcf, 0xfa, 0xed, 0xfe]
            | [0xfe, 0xed, 0xfa, 0xcf]
            | [0xce, 0xfa, 0xed, 0xfe]
            | [0xfe, 0xed, 0xfa, 0xce]
            | [0xca, 0xfe, 0xba, 0xbe]
            | [0xca, 0xfe, 0xba, 0xbf]
    ))
}

fn launcher_script() -> &'static str {
    r#"#!/bin/sh
SUPPORT="$HOME/Library/Application Support/GoghMode"
LOG_DIR="$HOME/Library/Logs"
mkdir -p "$LOG_DIR"
nohup /usr/bin/env -i HOME="$HOME" PATH="/usr/bin:/bin:/usr/sbin:/sbin" "$SUPPORT/goghmode-bin" >> "$LOG_DIR/GoghMode.log" 2>&1 &
"#
}

fn codesign_macos_binary_if_needed(path: &Path) -> anyhow::Result<()> {
    if !cfg!(target_os = "macos") || !is_macho_executable(path)? {
        return Ok(());
    }

    let output = Command::new("codesign")
        .arg("--force")
        .arg("--sign")
        .arg("-")
        .arg(path)
        .output()?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    anyhow::bail!("codesign failed for {}: {}", path.display(), stderr.trim());
}

fn info_plist() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>GoghMode</string>
    <key>CFBundleIdentifier</key>
    <string>dev.goghmode.app</string>
    <key>CFBundleName</key>
    <string>GoghMode</string>
    <key>CFBundleDisplayName</key>
    <string>GoghMode</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
"#
}
