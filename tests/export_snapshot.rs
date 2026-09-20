#![allow(dead_code)]

#[path = "../src/drawing.rs"]
mod drawing;
#[path = "../src/export.rs"]
mod export;
#[path = "../src/pages.rs"]
mod pages;
#[path = "../src/timeline.rs"]
mod timeline;

use drawing::{Drawing, CURRENT_SCHEMA_VERSION, DESKTOP_SCRATCH_PAGE_ID};
use drawing::{DrawingSnapshot, Narration, NarrationSegment};
use export::{render_step_crop, snapshot_to_rgba, step_window, write_artifacts};
use pages::{list_pages, page_id_is_safe, write_page};
use std::fs;

#[test]
fn multi_point_stroke_writes_json_svg_and_png() {
    let temp = tempfile::tempdir().unwrap();
    let mut drawing = Drawing::new(100.0, 80.0);
    drawing.begin_stroke(10.0, 12.0, 0.5, 1);
    drawing.push_point(40.0, 42.0, 0.5, 2);
    drawing.finish_stroke();

    let files = write_artifacts(
        &drawing.snapshot(),
        temp.path(),
        "latest",
        "drawings/",
        None,
    )
    .unwrap();

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

    let files = write_artifacts(
        &drawing.snapshot(),
        temp.path(),
        "latest",
        "drawings/",
        None,
    )
    .unwrap();

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

    let files = write_artifacts(
        &drawing.snapshot(),
        temp.path(),
        "latest",
        "drawings/",
        None,
    )
    .unwrap();

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

    write_artifacts(
        &drawing.snapshot(),
        temp.path(),
        "latest",
        "drawings/",
        None,
    )
    .unwrap();

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

    let index: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join("pages").join("index.json")).unwrap(),
    )
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
            ruling: Some(drawing::Ruling { style, spacing }),
        },
        strokes: Vec::new(),
        narration: None,
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
    let swift = std::fs::read_to_string("ipad-companion/GoghModeCompanion/DrawingSetStyle.swift")
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

/// A sheet someone spoke over. Strokes are `(x0, y0, x1, y1, started_at)`
/// lines; segments are `(start, end, text)`.
fn narrated_snapshot(
    canvas: (f32, f32),
    strokes: &[(f32, f32, f32, f32, u64)],
    segments: &[(u64, u64, &str)],
) -> DrawingSnapshot {
    DrawingSnapshot {
        schema_version: drawing::NARRATED_SCHEMA_VERSION,
        page: Some(drawing::PageRef {
            id: "talk".to_owned(),
            title: Some("Talk".to_owned()),
        }),
        canvas: drawing::CanvasSize {
            width: canvas.0,
            height: canvas.1,
            background: "#ffffff".to_owned(),
            ruling: None,
        },
        strokes: strokes
            .iter()
            .enumerate()
            .map(|(index, &(x0, y0, x1, y1, started_at))| drawing::Stroke {
                id: format!("stroke-{}", index + 1),
                color: "#111827".to_owned(),
                width: 4.0,
                points: vec![
                    drawing::Point {
                        x: x0,
                        y: y0,
                        pressure: 0.5,
                        t: 0,
                    },
                    drawing::Point {
                        x: x1,
                        y: y1,
                        pressure: 0.5,
                        t: 100,
                    },
                ],
                started_at: Some(started_at),
            })
            .collect(),
        narration: Some(Narration {
            language: "nl".to_owned(),
            engine: Some("test".to_owned()),
            segments: segments
                .iter()
                .map(|&(start, end, text)| NarrationSegment {
                    start,
                    end,
                    text: text.to_owned(),
                })
                .collect(),
        }),
    }
}

const HALO: [u8; 3] = [214, 228, 240];
const PAPER: [u8; 3] = [255, 255, 255];

fn rgb(image: &image::RgbaImage, x: u32, y: u32) -> [u8; 3] {
    let pixel = image.get_pixel(x, y);
    [pixel[0], pixel[1], pixel[2]]
}

fn png_colour_type(path: &std::path::Path) -> png::ColorType {
    let decoder = png::Decoder::new(std::io::BufReader::new(fs::File::open(path).unwrap()));
    decoder.read_info().unwrap().info().color_type
}

#[test]
fn narrated_page_writes_a_timeline_and_a_crop_per_step_and_mirrors_them_to_latest() {
    let temp = tempfile::tempdir().unwrap();
    let snapshot = narrated_snapshot(
        (400.0, 300.0),
        &[
            (20.0, 20.0, 60.0, 60.0, 1_500),
            (300.0, 200.0, 350.0, 250.0, 5_500),
        ],
        &[
            (1_000, 3_000, "Eerst de database."),
            (5_000, 7_000, "Dan de API."),
        ],
    );

    let files = write_page(&snapshot, temp.path()).unwrap();

    let page_dir = temp.path().join("pages").join("talk");
    assert!(page_dir.join("page.timeline.md").exists());
    assert!(page_dir.join("page.steps").join("001.png").exists());
    assert!(page_dir.join("page.steps").join("002.png").exists());
    assert!(!page_dir.join("page.steps").join("003.png").exists());
    assert_eq!(files.timeline, Some(temp.path().join("latest.timeline.md")));
    assert!(temp.path().join("latest.steps").join("002.png").exists());

    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(temp.path().join("latest.json")).unwrap())
            .unwrap();
    assert_eq!(json["files"]["timeline"], "drawings/latest.timeline.md");
    assert_eq!(json["files"]["steps"], "drawings/latest.steps/");
    assert_eq!(json["narration"]["segments"][1]["text"], "Dan de API.");
    assert_eq!(json["strokes"][0]["startedAt"], 1_500);

    let timeline = fs::read_to_string(temp.path().join("latest.timeline.md")).unwrap();
    assert!(timeline.starts_with("# Talk"));
    assert!(timeline.contains("## Step 1 · 00:00–00:02 · latest.steps/001.png"));
    assert!(timeline.contains("> Eerst de database."));
    assert!(timeline.contains("## Step 2 · 00:04–00:06 · latest.steps/002.png"));
    assert!(timeline.contains("> Dan de API."));
    assert!(timeline.contains("1 stroke, top-left"));
    assert!(timeline.contains("1 stroke, bottom-right"));
    assert_eq!(
        png_colour_type(&temp.path().join("latest.steps").join("001.png")),
        png::ColorType::Indexed
    );
}

/// The halo marks only the ink added in a step; every stroke keeps its own
/// colour, and a stroke from a later step is not on the sheet yet.
#[test]
fn a_crop_shows_the_sheet_as_it_stood_with_a_halo_under_the_new_ink_only() {
    let mut snapshot = narrated_snapshot(
        (200.0, 200.0),
        &[
            (20.0, 20.0, 60.0, 60.0, 1_500),
            (20.0, 60.0, 60.0, 20.0, 5_500),
        ],
        &[(1_000, 3_000, "Een lijn."), (5_000, 7_000, "En een kruis.")],
    );
    snapshot.strokes[0].color = "#cc0000".to_owned();
    let steps = timeline::build_steps(&snapshot, timeline::MAX_STEPS);
    assert_eq!(steps.len(), 2);

    let first_window = step_window(&snapshot, &steps[0]).unwrap();
    let first = render_step_crop(&snapshot, &steps[0], &[0], &first_window);
    let second_window = step_window(&snapshot, &steps[1]).unwrap();
    let second = render_step_crop(&snapshot, &steps[1], &[0, 1], &second_window);

    // Both windows start at the page corner and are not scaled, so page units are pixels.
    assert_eq!(
        (first_window.x, first_window.y, first_window.scale),
        (0.0, 0.0, 1.0)
    );
    assert_eq!(
        (second_window.x, second_window.y, second_window.scale),
        (0.0, 0.0, 1.0)
    );

    // A point on the second stroke, well clear of the first: not yet drawn in
    // the first crop, ink with a halo beside it in the second.
    assert_eq!(rgb(&first, 25, 55), PAPER);
    assert_eq!(rgb(&second, 25, 55), [17, 24, 39]);
    assert_eq!(rgb(&second, 30, 60), HALO);

    // The first stroke: haloed in its own step, then plain in its own red.
    assert_eq!(rgb(&first, 25, 25), [204, 0, 0]);
    assert_eq!(rgb(&first, 30, 20), HALO);
    assert_eq!(rgb(&second, 25, 25), [204, 0, 0]);
    assert_eq!(rgb(&second, 30, 20), PAPER);
}

#[test]
fn a_step_across_the_whole_page_is_scaled_down_to_the_crop_ceiling() {
    let snapshot = narrated_snapshot(
        (2000.0, 1500.0),
        &[(10.0, 10.0, 1990.0, 1490.0, 1_500)],
        &[(1_000, 3_000, "Alles.")],
    );
    let steps = timeline::build_steps(&snapshot, timeline::MAX_STEPS);

    let window = step_window(&snapshot, &steps[0]).unwrap();

    assert!(window.pixel_width() <= 512, "{}", window.pixel_width());
    assert!(window.pixel_height() <= 512, "{}", window.pixel_height());
    assert!(window.scale < 1.0);
    assert_eq!(window.region(2000.0, 1500.0), "centre");
}

#[test]
fn words_without_ink_fold_into_the_step_before_them_and_a_cap_merges_neighbours() {
    let snapshot = narrated_snapshot(
        (200.0, 200.0),
        &[
            (10.0, 10.0, 20.0, 20.0, 500),
            (10.0, 10.0, 20.0, 20.0, 3_500),
        ],
        &[
            (1_000, 2_000, "A"),
            (3_000, 4_000, "B"),
            (5_000, 6_000, "C"),
        ],
    );

    let steps = timeline::build_steps(&snapshot, timeline::MAX_STEPS);

    assert_eq!(steps.len(), 2);
    assert_eq!(
        (steps[0].segments.as_slice(), steps[0].strokes.as_slice()),
        (&[0][..], &[0][..])
    );
    assert_eq!(
        (steps[1].segments.as_slice(), steps[1].strokes.as_slice()),
        (&[1, 2][..], &[1][..])
    );

    let many: Vec<(f32, f32, f32, f32, u64)> = (0..150)
        .map(|index| (10.0, 10.0, 20.0, 20.0, 1_000 + index * 1_000 + 500))
        .collect();
    let spoken: Vec<String> = (0..150).map(|index| format!("zin {index}")).collect();
    let many_segments: Vec<(u64, u64, &str)> = spoken
        .iter()
        .enumerate()
        .map(|(index, text)| {
            (
                1_000 + index as u64 * 1_000,
                1_400 + index as u64 * 1_000,
                text.as_str(),
            )
        })
        .collect();
    let long = narrated_snapshot((200.0, 200.0), &many, &many_segments);

    assert_eq!(timeline::build_steps(&long, timeline::MAX_STEPS).len(), 150);
    let capped = timeline::build_steps(&long, 40);
    assert!(capped.len() <= 40, "{}", capped.len());
    assert_eq!(
        capped.iter().map(|step| step.segments.len()).sum::<usize>(),
        150
    );
    let windows: Vec<Option<timeline::Window>> =
        capped.iter().map(|step| step_window(&long, step)).collect();
    let markdown = timeline::markdown(&long, &capped, &windows, "latest");
    assert_eq!(markdown.matches("> zin ").count(), 150);
}

/// Whisper writes `***` for a stretch it heard nothing in. Nobody said that,
/// and in markdown a quoted `***` is a horizontal rule.
#[test]
fn a_silent_stretch_is_not_a_sentence_and_a_sheet_of_only_silence_is_plain() {
    let temp = tempfile::tempdir().unwrap();
    let spoken = narrated_snapshot(
        (200.0, 200.0),
        &[(20.0, 20.0, 60.0, 60.0, 5_500), (80.0, 80.0, 120.0, 120.0, 6_000)],
        &[(1_000, 2_000, "***"), (5_000, 7_000, "Dan de API.")],
    );

    write_artifacts(&spoken, temp.path(), "latest", "drawings/", None).unwrap();

    let timeline = fs::read_to_string(temp.path().join("latest.timeline.md")).unwrap();
    assert!(timeline.contains("## Step 1"));
    assert!(!timeline.contains("## Step 2"));
    assert!(timeline.contains("> Dan de API."));
    assert!(!timeline.contains("***"), "{timeline}");
    assert!(!fs::read_to_string(temp.path().join("latest.json")).unwrap().contains("***"));
    assert!(temp.path().join("latest.steps").join("001.png").exists());
    assert!(!temp.path().join("latest.steps").join("002.png").exists());

    let mut silence = spoken.clone();
    silence.narration.as_mut().unwrap().segments.truncate(1);
    let files = write_artifacts(&silence, temp.path(), "latest", "drawings/", None).unwrap();

    assert_eq!(files.timeline, None);
    assert!(!temp.path().join("latest.timeline.md").exists());
    assert!(!temp.path().join("latest.steps").exists());
    assert!(!fs::read_to_string(temp.path().join("latest.json")).unwrap().contains("narration"));
}

#[test]
fn a_plain_sheet_after_a_narrated_one_takes_the_words_away_with_it() {
    let temp = tempfile::tempdir().unwrap();
    let narrated = narrated_snapshot(
        (200.0, 200.0),
        &[(20.0, 20.0, 60.0, 60.0, 1_500)],
        &[(1_000, 3_000, "Eerst.")],
    );
    write_artifacts(&narrated, temp.path(), "latest", "drawings/", None).unwrap();
    assert!(temp.path().join("latest.timeline.md").exists());
    assert!(temp.path().join("latest.steps").join("001.png").exists());

    let mut plain = narrated.clone();
    plain.narration = None;
    let files = write_artifacts(&plain, temp.path(), "latest", "drawings/", None).unwrap();

    assert_eq!(files.timeline, None);
    assert!(!temp.path().join("latest.timeline.md").exists());
    assert!(!temp.path().join("latest.steps").exists());
    let json = fs::read_to_string(temp.path().join("latest.json")).unwrap();
    assert!(!json.contains("timeline"));
    let leftovers: Vec<String> = fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".tmp") || name.contains(".old"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}
