#![allow(dead_code)]

#[path = "../src/drawing.rs"]
mod drawing;
#[path = "../src/export.rs"]
mod export;
#[path = "../src/pages.rs"]
mod pages;

use drawing::{Drawing, CURRENT_SCHEMA_VERSION, DESKTOP_SCRATCH_PAGE_ID};
use export::{snapshot_to_rgba, write_snapshot};
use pages::{list_pages, page_id_is_safe, write_page};
use std::fs;

#[test]
fn multi_point_stroke_writes_json_svg_and_png() {
    let temp = tempfile::tempdir().unwrap();
    let mut drawing = Drawing::new(100.0, 80.0);
    drawing.begin_stroke(10.0, 12.0, 0.5, 1);
    drawing.push_point(40.0, 42.0, 0.5, 2);
    drawing.finish_stroke();

    let files = write_snapshot(&drawing.snapshot(), temp.path()).unwrap();

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

    let files = write_snapshot(&drawing.snapshot(), temp.path()).unwrap();

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

    let files = write_snapshot(&drawing.snapshot(), temp.path()).unwrap();

    let svg = fs::read_to_string(&files.svg).unwrap();
    assert!(svg.contains("<circle"));
    assert!(!svg.contains("90"));
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&files.json).unwrap()).unwrap();
    assert_eq!(json["strokes"].as_array().unwrap().len(), 1);
    assert_eq!(json["strokes"][0]["points"].as_array().unwrap().len(), 2);
}

#[test]
fn write_snapshot_leaves_no_temporary_files_after_success() {
    let temp = tempfile::tempdir().unwrap();
    let drawing = Drawing::new(25.0, 25.0);

    write_snapshot(&drawing.snapshot(), temp.path()).unwrap();

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
    write_snapshot(&snapshot, temp.path()).unwrap();

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
