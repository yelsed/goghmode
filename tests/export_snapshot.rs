#![allow(dead_code)]

#[path = "../src/drawing.rs"]
mod drawing;
#[path = "../src/export.rs"]
mod export;

use drawing::Drawing;
use export::{snapshot_to_rgba, write_snapshot};
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
    assert_eq!(json["schemaVersion"], 1);
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
