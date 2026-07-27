use std::fs;

/// Source greps, because egui has no practical snapshot testing here. They
/// cannot tell you the window looks right — only that the pieces the design
/// depends on have not quietly disappeared.
fn app_source() -> String {
    fs::read_to_string("src/app.rs").unwrap()
}

/// The URL moved off the permanent bar into the connection chip, but it must
/// still be reachable: it is the only way to connect a phone or anything else
/// that cannot pair.
#[test]
fn the_mobile_url_is_still_reachable_and_copyable() {
    let source = app_source();

    assert!(source.contains("Copy mobile URL"));
    assert!(source.contains("draw_connection_chip"));
    assert!(source.contains("Copied the mobile URL"));
}

#[test]
fn the_desktop_is_a_bridge_rather_than_a_drawing_surface() {
    let source = app_source();

    assert!(source.contains("draw_page_browser"), "the register is the home view");
    assert!(source.contains("draw_devices"), "pairing lives in the window");
    assert!(source.contains("StatusBar"));

    // The canvas and everything that only made sense beside it. Their absence
    // is the feature: the desktop owns the drawings directory and no longer
    // competes with the devices writing into it.
    for gone in [
        "draw_canvas",
        "fn paint_stroke",
        "Frame::canvas",
        "Send to agent",
        "Print prompt",
        "Copy image",
        "brush_width",
    ] {
        assert!(!source.contains(gone), "{gone} should have gone with the canvas");
    }
}

/// The appearance was unconditional because `configure_visuals` ran from `ui()`
/// every frame and hard-set `Visuals::dark()`. Registering both palettes once is
/// what lets the window follow the system instead.
#[test]
fn the_theme_is_installed_once_and_follows_the_system() {
    let source = app_source();

    assert!(source.contains("pub fn install_theme"));
    assert!(source.contains("ThemePreference::System"));
    assert!(source.contains("set_visuals_of(egui::Theme::Light"));
    assert!(source.contains("set_visuals_of(egui::Theme::Dark"));
    assert!(
        !source.contains("fn configure_visuals"),
        "the per-frame hard-coded dark theme should be gone"
    );
}

/// One mark, both appearances. A stamp that shifts with the theme is not a mark.
#[test]
fn the_issue_stamp_is_the_same_colour_in_both_appearances() {
    let source = app_source();
    let stamp_lines: Vec<&str> = source
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            // The value lines only — not the struct's field declaration.
            line.starts_with("stamp:") && line.contains("from_rgb")
        })
        .collect();

    assert_eq!(stamp_lines.len(), 2, "one stamp colour per palette");
    assert_eq!(
        stamp_lines[0].trim(),
        stamp_lines[1].trim(),
        "the stamp must not change with the appearance"
    );
}

/// Guards a bug that cost thirty-four processes in ten seconds.
///
/// A second launch used to "helpfully" run `tell application "GoghMode" to
/// activate`. That goes through LaunchServices, which runs the bundle launcher,
/// which starts another instance — which finds the port taken and activates
/// again. Nothing may relaunch the app by name from inside the app.
#[test]
fn a_second_launch_never_relaunches_the_application() {
    let main_source = fs::read_to_string("src/main.rs").unwrap();
    // Comments are stripped: the comment explaining this bug names the very
    // thing being banned, and it earns its place.
    let code: String = main_source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(!code.contains("to activate"));
    assert!(!code.contains("osascript"));
}

/// The window once painted from both palettes at once: a dark panel behind
/// white buttons and a white title block, with light-mode label grey on
/// near-black. The cause was two sources of truth — `ctx.theme()` for the
/// palette, the installed `Visuals` for everything egui draws itself.
///
/// Reading `dark_mode` off the visuals in play is what keeps them one.
#[test]
fn the_palette_follows_the_visuals_actually_being_painted() {
    let source = app_source();

    assert!(source.contains("ui.visuals().dark_mode"));
    assert!(
        !source.contains("ui.ctx().theme() =="),
        "the palette must not be chosen independently of the installed visuals"
    );
}

/// A QR code, a pairing payload and a device list are taller than the window.
/// Without somewhere to scroll they pushed the footer and the status line off
/// the bottom, where nothing could reach them.
#[test]
fn the_body_scrolls_so_the_footer_cannot_be_pushed_off_screen() {
    let source = app_source();

    assert!(source.contains("ScrollArea::vertical()"));
    assert!(source.contains("FOOTER_HEIGHT"));
}
