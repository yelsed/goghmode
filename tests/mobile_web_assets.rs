use std::fs;

fn read_asset(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn mobile_manifest_is_installable_standalone() {
    let manifest: serde_json::Value =
        serde_json::from_str(&read_asset("mobile/manifest.webmanifest"))
            .expect("mobile manifest should be valid JSON");

    assert_eq!(manifest["name"], "GoghMode Mobile");
    assert_eq!(manifest["short_name"], "GoghMode");
    assert_eq!(manifest["start_url"], "./");
    assert_eq!(manifest["scope"], "./");
    assert_eq!(manifest["display"], "standalone");
    assert_eq!(manifest["theme_color"], "#111827");
    assert!(manifest["icons"]
        .as_array()
        .expect("icons should be an array")
        .iter()
        .any(|icon| icon["src"] == "icon.svg"));

    assert_eq!(manifest["start_url"], "./");
    assert_eq!(manifest["scope"], "./");
}

#[test]
fn mobile_index_exports_existing_schema_contract() {
    let index = read_asset("mobile/index.html");

    assert!(index.contains("const SCHEMA_VERSION = 1"));
    assert!(index.contains("schemaVersion"));
    assert!(index.contains("drawings/latest.json"));
    assert!(index.contains("drawings/latest.svg"));
    assert!(index.contains("drawings/latest.png"));
    assert!(index.contains("function snapshot()"));
    assert!(index.contains("function snapshotToSvg(snapshot)"));
    assert!(index.contains("goghmode-latest.json"));
    assert!(index.contains("goghmode-latest.svg"));
    assert!(index.contains("goghmode-latest.png"));
}

#[test]
fn mobile_index_can_send_snapshot_to_desktop_app() {
    let index = read_asset("mobile/index.html");

    assert!(index.contains("Send to Mac"));
    assert!(index.contains("async function sendToMac()"));
    assert!(index.contains("fetch(\"save\""));
    assert!(index.contains("method: \"POST\""));
    assert!(index.contains("Saved to Mac"));
}
#[test]
fn mobile_index_uses_pointer_events_and_share_fallback() {
    let index = read_asset("mobile/index.html");

    assert!(index.contains("pointerdown"));
    assert!(index.contains("pointermove"));
    assert!(index.contains("pointerup"));
    assert!(index.contains("pointercancel"));
    assert!(index.contains("touch-action: none"));
    assert!(index.contains("preventDefault"));
    assert!(index.contains("navigator.canShare"));
    assert!(index.contains("serviceWorker"));
    assert!(index.contains("service-worker.js"));
    assert!(index.contains("Sharing unavailable; use Export PNG."));
}

#[test]
fn mobile_index_prevents_text_selection_during_drawing() {
    let index = read_asset("mobile/index.html");

    assert!(index.contains("-webkit-user-select: none"));
    assert!(index.contains("user-select: none"));
    assert!(index.contains("-webkit-touch-callout: none"));
    assert!(index.contains("selectstart"));
}

#[test]
fn mobile_index_uses_polished_product_styling() {
    let index = read_asset("mobile/index.html");

    assert!(index.contains("oklch("));
    assert!(index.contains("class=\"primary\""));
    assert!(index.contains("class=\"secondary-actions\""));
    assert!(index.contains("aria-label=\"Brush size\""));
}

#[test]
fn mobile_service_worker_caches_app_shell_only() {
    let service_worker = read_asset("mobile/service-worker.js");

    assert!(service_worker.contains("goghmode-mobile-v1"));
    assert!(service_worker.contains("APP_SHELL"));
    assert!(service_worker.contains("manifest.webmanifest"));
    assert!(service_worker.contains("icon.svg"));
    assert!(!service_worker.contains("goghmode-latest"));
    assert!(!service_worker.contains("drawings/latest"));
}
