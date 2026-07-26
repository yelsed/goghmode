#[path = "../src/app_install.rs"]
mod app_install;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn files_under(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_under(&path));
        } else {
            found.push(path);
        }
    }
    found
}

#[test]
fn linux_install_writes_a_desktop_entry_and_icon() {
    let home = tempfile::tempdir().unwrap();
    let executable = home.path().join("bin").join("goghmode");

    let entry_path = app_install::linux::install(home.path(), &executable).unwrap();

    assert_eq!(entry_path, app_install::linux::desktop_entry_path(home.path()));
    let entry = fs::read_to_string(&entry_path).unwrap();
    assert!(entry.starts_with("[Desktop Entry]"));
    assert!(entry.contains("Type=Application"));
    assert!(entry.contains("Name=GoghMode"));
    assert!(entry.contains("Icon=goghmode"));
    assert!(entry.contains("Terminal=false"));
    // Quoted, so a home directory containing a space does not turn the path into
    // an executable plus an argument.
    assert!(entry.contains(&format!("Exec=\"{}\"", executable.display())));

    let icon = fs::read(app_install::linux::desktop_icon_path(home.path())).unwrap();
    assert!(!icon.is_empty());
}

/// Omarchy owns `~/.local/share/omarchy/`. Writing anywhere near it — or
/// anywhere outside the two standard directories — is the failure this guards.
#[test]
fn linux_install_touches_only_standard_per_user_directories() {
    let home = tempfile::tempdir().unwrap();

    app_install::linux::install(home.path(), Path::new("/usr/bin/goghmode")).unwrap();

    let applications = home.path().join(".local").join("share").join("applications");
    let icons = home.path().join(".local").join("share").join("icons");
    for path in files_under(home.path()) {
        assert!(
            path.starts_with(&applications) || path.starts_with(&icons),
            "installer wrote outside the standard directories: {}",
            path.display()
        );
    }
    assert!(!home
        .path()
        .join(".local")
        .join("share")
        .join("omarchy")
        .exists());
}

#[test]
fn linux_install_is_repeatable() {
    let home = tempfile::tempdir().unwrap();

    app_install::linux::install(home.path(), Path::new("/usr/bin/goghmode")).unwrap();
    let entry_path =
        app_install::linux::install(home.path(), Path::new("/opt/goghmode/goghmode")).unwrap();

    let entry = fs::read_to_string(&entry_path).unwrap();
    assert!(entry.contains("Exec=\"/opt/goghmode/goghmode\""));
    assert!(!entry.contains("/usr/bin/goghmode"));
}

#[test]
fn installer_only_codesigns_macho_binaries() {
    let home = tempfile::tempdir().unwrap();
    let fake_binary = home.path().join("fake");
    let macho_binary = home.path().join("macho");

    fs::write(&fake_binary, b"#!/bin/sh\n").unwrap();
    fs::write(&macho_binary, [0xcf, 0xfa, 0xed, 0xfe, 0, 0, 0, 0]).unwrap();

    assert!(!app_install::is_macho_executable(&fake_binary).unwrap());
    assert!(app_install::is_macho_executable(&macho_binary).unwrap());
}
#[test]
fn app_bundle_path_lives_in_user_applications() {
    let home = tempfile::tempdir().unwrap();

    let path = app_install::app_bundle_path(home.path());

    assert_eq!(path, home.path().join("Applications").join("GoghMode.app"));
}

#[test]
fn app_bundle_launcher_starts_binary_outside_launchservices_app_context() {
    let home = tempfile::tempdir().unwrap();
    let source_binary = home.path().join("source-goghmode");
    fs::write(&source_binary, b"fake binary").unwrap();
    fs::set_permissions(&source_binary, fs::Permissions::from_mode(0o755)).unwrap();

    let bundle = app_install::install_macos_app(home.path(), &source_binary).unwrap();
    let launcher = bundle.join("Contents").join("MacOS").join("GoghMode");
    let support_binary = home
        .path()
        .join("Library")
        .join("Application Support")
        .join("GoghMode")
        .join("goghmode-bin");
    let launcher_text = fs::read_to_string(&launcher).unwrap();

    assert_eq!(fs::read(&support_binary).unwrap(), b"fake binary");
    assert!(!bundle
        .join("Contents")
        .join("MacOS")
        .join("goghmode-bin")
        .exists());
    assert!(launcher_text.contains("Application Support/GoghMode"));
    assert!(launcher_text.contains("goghmode-bin"));
    // The launcher must not name a drawings directory. The binary's own default
    // is the single source of truth, so a Spotlight launch and a terminal launch
    // cannot drift apart.
    assert!(!launcher_text.contains("--drawings-dir"));
    // env -i wipes the environment, so HOME has to survive for the default to resolve.
    assert!(launcher_text.contains("HOME=\"$HOME\""));
    assert!(launcher_text.contains("&"));
    assert!(launcher_text.contains("env -i"));
    assert!(launcher_text.contains("nohup"));
    assert!(!launcher_text.contains("exec "));
}

#[test]
fn install_macos_app_creates_spotlight_visible_bundle() {
    let home = tempfile::tempdir().unwrap();
    let source_binary = home.path().join("source-goghmode");
    fs::write(&source_binary, b"fake binary").unwrap();
    fs::set_permissions(&source_binary, fs::Permissions::from_mode(0o755)).unwrap();

    let bundle = app_install::install_macos_app(home.path(), &source_binary).unwrap();

    assert_eq!(
        bundle,
        home.path().join("Applications").join("GoghMode.app")
    );

    let info = fs::read_to_string(bundle.join("Contents").join("Info.plist")).unwrap();
    assert!(info.contains("<key>CFBundleName</key>"));
    assert!(info.contains("<string>GoghMode</string>"));
    assert!(info.contains("<key>CFBundleExecutable</key>"));
    assert!(info.contains("<string>GoghMode</string>"));

    let support_binary = home
        .path()
        .join("Library")
        .join("Application Support")
        .join("GoghMode")
        .join("goghmode-bin");
    assert_eq!(fs::read(&support_binary).unwrap(), b"fake binary");
    let executable = bundle.join("Contents").join("MacOS").join("GoghMode");
    assert_ne!(
        fs::metadata(&executable).unwrap().permissions().mode() & 0o111,
        0
    );
}
