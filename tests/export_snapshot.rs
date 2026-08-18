#![allow(dead_code)]

#[path = "../src/drawing.rs"]
mod drawing;
#[path = "../src/export.rs"]
mod export;
#[path = "../src/pages.rs"]
mod pages;

use drawing::{Drawing, CURRENT_SCHEMA_VERSION, DESKTOP_SCRATCH_PAGE_ID};
use export::{snapshot_to_rgba, write_artifacts};
use pages::{list_pages, page_id_is_safe, write_page};
use std::fs;

#[test]
fn multi_point_stroke_writes_json_svg_and_png() {
    let temp = tempfile::tempdir().unwrap();
    let mut drawing = Drawing::new(100.0, 80.0);
    drawing.begin_stroke(10.0, 12.0, 0.5, 1);
    drawing.push_point(40.0, 42.0, 0.5, 2);
    drawing.finish_stroke();

    let files = write_artifacts(&drawing.snapshot(), temp.path(), "latest", "drawings/", None).unwrap();

    assert!(files.json.exists());
    assert!(files.svg.exists());
    assert!(files.png.exists());

    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&files.json).unwrap()).unwrap();
    assert_eq!(json["schemaVersion"], CURRENT_SCHEMA_VERSION);
    assert_eq!(json["files"]["svg"], "drawings/latest.svg");
    assert_eq!(json["strokes"].as_array().unwrap().len(), 1);

    let svg = fs::read_to_string(&files.svg).unwrap();
    assert!(svg.contains("<path"));
    assert!(svg.contains("stroke=\"#111827\""));
    assert!(!svg.contains("<script"));
}

#[test]
fn empty_drawing_writes_valid_white_svg_and_png() {
    let temp = tempfile::tempdir().unwrap();
    let drawing = Drawing::new(100.0, 80.0);

    let files = write_artifacts(&drawing.snapshot(), temp.path(), "latest", "drawings/", None).unwrap();

    assert!(files.json.exists());
    assert!(files.svg.exists());
    assert!(files.png.exists());
    let svg = fs::read_to_string(&files.svg).unwrap();
    assert!(svg.contains("<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>"));
}

#[test]
fn resized_drawing_omits_out_of_bounds_points_from_svg_but_keeps_json_stroke() {
    let temp = tempfile::tempdir().unwrap();
    let mut drawing = Drawing::new(100.0, 100.0);
    drawing.begin_stroke(10.0, 10.0, 0.5, 1);
    drawing.push_point(90.0, 90.0, 0.5, 2);
    drawing.finish_stroke();
    drawing.set_canvas_size(50.0, 50.0);

    let files = write_artifacts(&drawing.snapshot(), temp.path(), "latest", "drawings/", None).unwrap();

    let svg = fs::read_to_string(&files.svg).unwrap();
    assert!(svg.contains("<circle"));
    assert!(!svg.contains("90"));
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&files.json).unwrap()).unwrap();
    assert_eq!(json["strokes"].as_array().unwrap().len(), 1);
    assert_eq!(json["strokes"][0]["points"].as_array().unwrap().len(), 2);
}

#[test]
fn writing_latest_leaves_no_temporary_files_after_success() {
    let temp = tempfile::tempdir().unwrap();
    let drawing = Drawing::new(25.0, 25.0);

    write_artifacts(&drawing.snapshot(), temp.path(), "latest", "drawings/", None).unwrap();

    let temporary_count = fs::read_dir(temp.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(temporary_count, 0);
}

/// The constant lost its Mac-specific name; the value must not follow it. It is
/// a directory name under `drawings/pages/`, so changing it orphans every
/// scratch page already on disk — silent data loss wearing a cleanup's clothes.
#[test]
fn desktop_scratch_page_keeps_its_on_disk_identifier() {
    assert_eq!(DESKTOP_SCRATCH_PAGE_ID, "mac-scratch");
}

#[test]
fn desktop_canvas_writes_its_own_page_instead_of_only_latest() {
    let temp = tempfile::tempdir().unwrap();
    let mut drawing = Drawing::new(60.0, 40.0);
    drawing.begin_stroke(5.0, 5.0, 0.5, 1);
    drawing.push_point(30.0, 20.0, 0.5, 2);
    drawing.finish_stroke();

    write_page(&drawing.snapshot(), temp.path()).unwrap();

    let page_dir = temp.path().join("pages").join(DESKTOP_SCRATCH_PAGE_ID);
    assert!(page_dir.join("page.json").exists());
    assert!(page_dir.join("page.svg").exists());
    assert!(page_dir.join("page.png").exists());
    assert!(temp.path().join("latest.json").exists());
}

#[test]
fn rebuilt_index_lists_every_page_newest_first() {
    let temp = tempfile::tempdir().unwrap();
    let mut drawing = Drawing::new(40.0, 40.0);
    drawing.begin_stroke(2.0, 2.0, 0.5, 1);
    drawing.push_point(20.0, 20.0, 0.5, 2);
    drawing.finish_stroke();

    let mut snapshot = drawing.snapshot();
    snapshot.page = Some(drawing::PageRef {
        id: "older".to_owned(),
        title: Some("Older".to_owned()),
    });
    write_page(&snapshot, temp.path()).unwrap();
    snapshot.page = Some(drawing::PageRef {
        id: "newer".to_owned(),
        title: None,
    });
    write_page(&snapshot, temp.path()).unwrap();

    let pages = list_pages(temp.path());

    assert_eq!(pages.len(), 2);
    assert!(pages[0].updated_at >= pages[1].updated_at);
    assert_eq!(pages.iter().filter(|page| page.stroke_count == 1).count(), 2);
    assert_eq!(
        pages
            .iter()
            .find(|page| page.page_id == "older")
            .and_then(|page| page.title.clone()),
        Some("Older".to_owned())
    );

    let index: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(temp.path().join("pages").join("index.json")).unwrap())
            .unwrap();
    assert_eq!(index["pages"].as_array().unwrap().len(), 2);
}

#[test]
fn page_ids_are_restricted_to_names_that_cannot_leave_the_pages_directory() {
    assert!(page_id_is_safe("note-1"));
    assert!(page_id_is_safe("A_b-9"));

    assert!(!page_id_is_safe(""));
    assert!(!page_id_is_safe("../escape"));
    assert!(!page_id_is_safe("a/b"));
    assert!(!page_id_is_safe("."));
    assert!(!page_id_is_safe(&"x".repeat(65)));
}

#[test]
fn png_export_keeps_the_stroke_colour_the_svg_already_honoured() {
    let temp = tempfile::tempdir().unwrap();
    let mut drawing = Drawing::new(30.0, 20.0);
    drawing.begin_stroke(2.0, 2.0, 0.5, 1);
    drawing.push_point(25.0, 15.0, 0.5, 2);
    drawing.finish_stroke();

    let mut snapshot = drawing.snapshot();
    snapshot.strokes[0].color = "#cc0000".to_owned();
    write_artifacts(&snapshot, temp.path(), "latest", "drawings/", None).unwrap();

    let image = snapshot_to_rgba(&snapshot);

    assert!(
        image
            .pixels()
            .any(|pixel| pixel.0 == [204, 0, 0, 255]),
        "a red stroke should produce red pixels, not the default ink"
    );
}

#[test]
fn snapshot_to_rgba_matches_canvas_dimensions_and_draws_dark_pixels() {
    let mut drawing = Drawing::new(10.2, 6.1);
    drawing.begin_stroke(1.0, 1.0, 0.5, 1);
    drawing.push_point(8.0, 5.0, 0.5, 2);
    drawing.finish_stroke();

    let image = snapshot_to_rgba(&drawing.snapshot());

    assert_eq!(image.width(), 11);
    assert_eq!(image.height(), 7);
    assert!(image
        .pixels()
        .any(|pixel| pixel.0[0] < 64 && pixel.0[1] < 64 && pixel.0[2] < 64));
}

#[test]
fn sheet_numbers_follow_creation_order_not_the_order_pages_were_last_edited() {
    let temp = tempfile::tempdir().unwrap();
    let mut drawing = Drawing::new(40.0, 40.0);
    drawing.begin_stroke(2.0, 2.0, 0.5, 1);
    drawing.push_point(20.0, 20.0, 0.5, 2);
    drawing.finish_stroke();

    let mut snapshot = drawing.snapshot();
    for page_id in ["first", "second", "third"] {
        snapshot.page = Some(drawing::PageRef {
            id: page_id.to_owned(),
            title: None,
        });
        pages::write_page(&snapshot, temp.path()).unwrap();
    }

    let before = pages::sheet_numbers(&list_pages(temp.path()));

    // Editing the oldest page moves it to the top of the register; its sheet
    // number must not follow it, or every number on screen reshuffles whenever
    // any sheet is touched.
    snapshot.page = Some(drawing::PageRef {
        id: "first".to_owned(),
        title: None,
    });
    pages::write_page(&snapshot, temp.path()).unwrap();
    let after = pages::sheet_numbers(&list_pages(temp.path()));

    assert_eq!(before, after);
    assert_eq!(before.len(), 3);
}

#[test]
fn stamping_a_sheet_keeps_the_time_it_was_drawn() {
    let temp = tempfile::tempdir().unwrap();
    let mut drawing = Drawing::new(40.0, 30.0);
    drawing.begin_stroke(5.0, 5.0, 0.5, 1);
    drawing.push_point(20.0, 20.0, 0.5, 2);
    drawing.finish_stroke();

    let mut snapshot = drawing.snapshot();
    snapshot.page = Some(drawing::PageRef {
        id: "drawn-earlier".to_owned(),
        title: None,
    });
    write_page(&snapshot, temp.path()).unwrap();

    // Long enough that a stamp minting its own time cannot land on the same
    // millisecond as the page it is mirroring.
    std::thread::sleep(std::time::Duration::from_millis(20));
    pages::promote_page(temp.path(), "drawn-earlier").unwrap();

    let latest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(temp.path().join("latest.json")).unwrap()).unwrap();
    // Read the page's own copy rather than what `write_page` returned: with
    // nothing pinned that return value is the mirror itself, so comparing
    // against it compares `latest.json` with itself and passes either way.
    let stored: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            temp.path()
                .join("pages")
                .join("drawn-earlier")
                .join("page.json"),
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        latest["updatedAt"], stored["updatedAt"],
        "a stamped sheet must keep its own time, or an old sketch reads as freshly drawn"
    );
}

/// The ruling is a writing aid the person chose, and the point of baking it in is
/// that the page the agent reads is the page that was drawn on.
fn ruled_snapshot(style: drawing::RulingStyle, spacing: f32) -> drawing::DrawingSnapshot {
    drawing::DrawingSnapshot {
        schema_version: CURRENT_SCHEMA_VERSION,
        page: None,
        canvas: drawing::CanvasSize {
            width: 100.0,
            height: 100.0,
            background: "#ffffff".to_owned(),
            ruling: Some(drawing::Ruling {
                style,
                spacing,
            }),
        },
        strokes: Vec::new(),
    }
}

#[test]
fn a_ruled_sheet_carries_its_rules_into_the_png_and_the_svg() {
    let snapshot = ruled_snapshot(drawing::RulingStyle::Grid, 32.0);

    let svg = export::snapshot_to_svg(&snapshot);
    assert!(svg.contains("#C9C4BB"), "ruling ink missing from the svg");
    assert!(svg.contains("<line"), "grid ruling should be drawn as lines");

    let image = snapshot_to_rgba(&snapshot);
    let ruled = image.get_pixel(10, 32);
    let blank = image.get_pixel(10, 16);
    assert_eq!(ruled.0, [201, 196, 187, 255], "no rule where one belongs");
    assert_eq!(blank.0, [255, 255, 255, 255], "a rule where none belongs");
}

#[test]
fn a_plain_sheet_exports_exactly_as_it_always_did() {
    let mut snapshot = ruled_snapshot(drawing::RulingStyle::Grid, 32.0);
    snapshot.canvas.ruling = None;

    let svg = export::snapshot_to_svg(&snapshot);
    assert!(!svg.contains("<line"), "an unruled sheet gained rules");

    let image = snapshot_to_rgba(&snapshot);
    assert_eq!(image.get_pixel(10, 32).0, [255, 255, 255, 255]);

    let temp = tempfile::tempdir().unwrap();
    write_artifacts(&snapshot, temp.path(), "latest", "drawings/", None).unwrap();
    let json = fs::read_to_string(temp.path().join("latest.json")).unwrap();
    assert!(
        !json.contains("ruling"),
        "an unruled sheet should not mention ruling at all"
    );
}

#[test]
fn ruled_lines_stay_inside_the_page() {
    let snapshot = ruled_snapshot(drawing::RulingStyle::Lines, 100.0);

    // The only stop would be at the page edge itself, which is not a rule.
    let svg = export::snapshot_to_svg(&snapshot);
    assert!(!svg.contains("<line"));

    let image = snapshot_to_rgba(&snapshot);
    assert_eq!(image.get_pixel(10, 99).0, [255, 255, 255, 255]);
}

#[test]
fn dotted_ruling_marks_the_crossings_rather_than_drawing_lines() {
    let snapshot = ruled_snapshot(drawing::RulingStyle::Dots, 25.0);

    let svg = export::snapshot_to_svg(&snapshot);
    assert!(svg.contains("<circle"), "dots should be drawn as circles");
    assert!(!svg.contains("<line"));

    let image = snapshot_to_rgba(&snapshot);
    assert_eq!(image.get_pixel(25, 25).0, [201, 196, 187, 255]);
    assert_eq!(image.get_pixel(12, 12).0, [255, 255, 255, 255]);
}

/// The one thing that cannot be shared across the language boundary, so it is
/// guarded instead.
///
/// The iPad draws the ruling on screen and this crate draws it again into the
/// PNG the agent reads. Those are two implementations of one appearance, and the
/// whole promise of ruling is that the page the agent reads is the page that was
/// written on. If the two inks drift, nothing else notices.
///
/// A source grep for the same reason `tests/app_mobile_url.rs` uses one: there is
/// no way to link the two and no way to snapshot them together.
#[test]
fn the_ipad_rules_a_sheet_in_the_same_ink_the_exporter_bakes_in() {
    let swift = std::fs::read_to_string(
        "ipad-companion/GoghModeCompanion/DrawingSetStyle.swift",
    )
    .expect("the companion's tokens should be readable from the repository root");

    // #C9C4BB, which is `rule-hair` in DESIGN.md. The companion resolves its
    // ruling ink from that token rather than retyping the triple, so the token is
    // what has to match.
    assert!(
        swift.contains("ruleHair = dynamic(light: (0.788, 0.769, 0.733)"),
        "Sheet.ruleHair no longer matches the exporter's RULING_INK"
    );
    assert!(
        swift.contains("rulingInk = UIColor(Sheet.ruleHair)"),
        "Sheet.rulingInk should stay derived from the rule-hair token"
    );

    let expected: [u8; 3] = [
        (0.788 * 255.0_f32).round() as u8,
        (0.769 * 255.0_f32).round() as u8,
        (0.733 * 255.0_f32).round() as u8,
    ];
    let snapshot = ruled_snapshot(drawing::RulingStyle::Lines, 32.0);
    let image = snapshot_to_rgba(&snapshot);
    let rule = image.get_pixel(10, 32).0;

    assert_eq!([rule[0], rule[1], rule[2]], expected);
}
