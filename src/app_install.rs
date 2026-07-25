use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn app_bundle_path(home_dir: &Path) -> PathBuf {
    home_dir.join("Applications").join("GoghMode.app")
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
