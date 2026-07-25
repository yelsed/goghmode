#[path = "../src/app_install.rs"]
mod app_install;

use std::fs;
use std::os::unix::fs::PermissionsExt;

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
