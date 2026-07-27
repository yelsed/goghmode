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
/// every frame and hard-set `Visuals::dark()`. Installing once is what makes it
/// deliberate.
#[test]
fn the_theme_is_installed_once_and_pinned() {
    let source = app_source();

    assert!(source.contains("pub fn install_theme"));
    assert!(source.contains("ThemePreference::Light"));
    assert!(
        !source.contains("fn configure_visuals"),
        "the per-frame hard-coded theme should be gone"
    );
    assert!(
        !source.contains("ThemePreference::System"),
        "following the system needs the palette, the visuals and the clear \
         colour to agree; two attempts at that shipped unreadable windows"
    );
}

/// The bug that made half the window unreadable, and the one that source greps
/// can actually catch.
///
/// egui was painting light the whole time — `dark_mode=false`,
/// `panel_fill=#EDEAE4`, ink `#1A1917`. The black came from the window's clear
/// colour, which is a separate thing from the panel fill and defaults dark. So
/// paper-coloured ink landed on a black surface.
#[test]
fn the_window_clear_colour_matches_the_ground() {
    let source = app_source();

    assert!(
        source.contains("fn clear_color"),
        "without this the window paints its own background, and it is not paper"
    );
    assert!(source.contains("sheet().ground.to_normalized_gamma_f32()"));
}

/// One palette. Two of them meant the palette, the visuals and the clear colour
/// all had to agree, and they did not.
#[test]
fn there_is_exactly_one_palette() {
    let source = app_source();

    assert!(source.contains("const SET: Sheet"));
    assert!(!source.contains("const DARK: Sheet"));
    assert!(!source.contains("ui.visuals().dark_mode"));
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

/// A QR code, a pairing payload and a device list are taller than the window.
/// Without somewhere to scroll they pushed the footer and the status line off
/// the bottom, where nothing could reach them.
#[test]
fn the_body_scrolls_so_the_footer_cannot_be_pushed_off_screen() {
    let source = app_source();

    assert!(source.contains("ScrollArea::vertical()"));
    assert!(source.contains("FOOTER_HEIGHT"));
}
