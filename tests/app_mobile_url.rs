use std::fs;

#[test]
fn desktop_toolbar_exposes_copyable_mobile_url() {
    let app_source = fs::read_to_string("src/app.rs").unwrap();

    assert!(app_source.contains("Copy mobile URL"));
    assert!(app_source.contains("copy_mobile_url"));
    assert!(app_source.contains("Copied mobile URL to clipboard"));
}

#[test]
fn desktop_app_uses_structured_polished_layout() {
    let app_source = fs::read_to_string("src/app.rs").unwrap();

    assert!(app_source.contains("configure_visuals"));
    assert!(app_source.contains("draw_toolbar"));
    assert!(app_source.contains("draw_canvas"));
    assert!(app_source.contains("Send to Claude"));
    assert!(app_source.contains("StatusBar"));
    assert!(app_source.contains("Frame::canvas"));
}
