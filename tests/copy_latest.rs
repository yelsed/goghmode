#![allow(dead_code)]

#[path = "../src/clipboard.rs"]
mod clipboard;
#[path = "../src/drawing.rs"]
mod drawing;
#[path = "../src/export.rs"]
mod export;
#[path = "../src/pages.rs"]
mod pages;
#[path = "../src/raycast.rs"]
mod raycast;

use clipboard::{
    describe_age, read_sheet_image, read_updated_at, sheet_image_path, sheet_json_path,
};
use drawing::{Drawing, PageRef};
use std::fs;
use std::path::Path;

fn write_sheet(drawings_dir: &Path, page_id: &str) {
    let mut drawing = Drawing::new(60.0, 40.0);
    drawing.begin_stroke(5.0, 5.0, 0.5, 1);
    drawing.push_point(50.0, 30.0, 0.5, 2);
    drawing.finish_stroke();

    let mut snapshot = drawing.snapshot();
    snapshot.page = Some(PageRef {
        id: page_id.to_owned(),
        title: None,
    });
    pages::write_page(&snapshot, drawings_dir).unwrap();
}

#[test]
fn a_copy_without_a_page_reads_the_latest_sheet() {
    let drawings_dir = tempfile::tempdir().unwrap();

    let path = sheet_image_path(drawings_dir.path(), None).unwrap();

    assert_eq!(path, drawings_dir.path().join("latest.png"));
}

#[test]
fn a_named_page_reads_that_sheets_own_copy() {
    let drawings_dir = tempfile::tempdir().unwrap();

    let path = sheet_image_path(drawings_dir.path(), Some("abc123")).unwrap();

    assert_eq!(
        path,
        drawings_dir
            .path()
            .join("pages")
            .join("abc123")
            .join("page.png")
    );
}

/// The page id comes off the command line, so a copy is the same trust boundary
/// as a save: refuse before joining, not after.
#[test]
fn a_page_id_that_could_escape_the_drawings_directory_is_refused() {
    let drawings_dir = tempfile::tempdir().unwrap();

    for page_id in ["../escape", "..", "a/b", &"x".repeat(65), ""] {
        let result = sheet_image_path(drawings_dir.path(), Some(page_id));

        assert!(result.is_err(), "expected {page_id:?} to be refused");
    }
}

#[test]
fn an_ordinary_page_id_is_still_accepted() {
    let drawings_dir = tempfile::tempdir().unwrap();

    assert!(sheet_image_path(drawings_dir.path(), Some("mac-scratch")).is_ok());
    assert!(sheet_image_path(drawings_dir.path(), Some(&"x".repeat(64))).is_ok());
}

#[test]
fn a_missing_sheet_names_the_file_it_looked_for() {
    let drawings_dir = tempfile::tempdir().unwrap();
    let path = sheet_image_path(drawings_dir.path(), None).unwrap();

    let Err(error) = read_sheet_image(&path) else {
        panic!("an empty drawings directory must not read as an image");
    };
    let error = error.to_string();

    assert!(error.contains("No sheet at"), "unexpected message: {error}");
    assert!(error.contains("latest.png"), "unexpected message: {error}");
}

#[test]
fn a_written_sheet_decodes_to_the_canvas_size_in_rgba() {
    let drawings_dir = tempfile::tempdir().unwrap();
    write_sheet(drawings_dir.path(), "mac-scratch");

    let image = read_sheet_image(&sheet_image_path(drawings_dir.path(), None).unwrap()).unwrap();

    assert_eq!(image.width, 60);
    assert_eq!(image.height, 40);
    assert_eq!(image.rgba.len(), 60 * 40 * 4);
}

#[test]
fn a_named_page_decodes_from_its_own_directory() {
    let drawings_dir = tempfile::tempdir().unwrap();
    write_sheet(drawings_dir.path(), "ipad-one");

    let path = sheet_image_path(drawings_dir.path(), Some("ipad-one")).unwrap();
    let image = read_sheet_image(&path).unwrap();

    assert_eq!(image.width, 60);
    assert_eq!(image.rgba.len(), 60 * 40 * 4);
}

#[test]
fn the_age_comes_from_the_stamp_beside_the_image() {
    let drawings_dir = tempfile::tempdir().unwrap();
    write_sheet(drawings_dir.path(), "mac-scratch");
    let image_path = sheet_image_path(drawings_dir.path(), None).unwrap();

    let updated_at = read_updated_at(&sheet_json_path(&image_path)).unwrap();

    assert_eq!(
        sheet_json_path(&image_path),
        drawings_dir.path().join("latest.json")
    );
    assert!(updated_at > 0);
}

/// The stamp is the extra, not the point. Losing it must not lose the image.
#[test]
fn a_missing_stamp_leaves_the_image_readable() {
    let drawings_dir = tempfile::tempdir().unwrap();
    write_sheet(drawings_dir.path(), "mac-scratch");
    let image_path = sheet_image_path(drawings_dir.path(), None).unwrap();
    fs::remove_file(sheet_json_path(&image_path)).unwrap();

    assert!(read_updated_at(&sheet_json_path(&image_path)).is_none());
    assert!(read_sheet_image(&image_path).is_ok());
}

#[test]
fn an_unreadable_stamp_is_treated_as_no_stamp() {
    let drawings_dir = tempfile::tempdir().unwrap();
    let json_path = drawings_dir.path().join("latest.json");
    fs::write(&json_path, "{ not json").unwrap();

    assert!(read_updated_at(&json_path).is_none());
}

#[test]
fn a_sheet_stamped_moments_ago_reads_as_just_now() {
    let now = 1_700_000_000_000_u128;

    assert_eq!(describe_age(now - 5_000, now), "just now");
    assert_eq!(describe_age(now - 59_000, now), "just now");
}

#[test]
fn an_older_sheet_reads_in_minutes_hours_then_days() {
    let now = 1_700_000_000_000_u128;

    assert_eq!(describe_age(now - 60_000, now), "1 minute ago");
    assert_eq!(describe_age(now - 3 * 60_000, now), "3 minutes ago");
    assert_eq!(describe_age(now - 60 * 60_000, now), "1 hour ago");
    assert_eq!(describe_age(now - 5 * 60 * 60_000, now), "5 hours ago");
    assert_eq!(describe_age(now - 24 * 60 * 60_000, now), "1 day ago");
    assert_eq!(describe_age(now - 3 * 24 * 60 * 60_000, now), "3 days ago");
}

/// Two clocks that disagree would otherwise wrap the subtraction and report a
/// sheet as hundreds of millions of years old.
#[test]
fn a_stamp_from_the_future_reads_as_just_now() {
    let now = 1_700_000_000_000_u128;

    assert_eq!(describe_age(now + 60_000, now), "just now");
}

#[test]
fn the_installed_raycast_script_is_executable_and_calls_copy() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().unwrap();

    let path = raycast::install_raycast_script(directory.path()).unwrap();
    let contents = fs::read_to_string(&path).unwrap();

    assert_eq!(path, directory.path().join("copy-latest-sheet.sh"));
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert!(contents.contains("@raycast.title Copy latest GoghMode sheet"));
    assert!(contents.contains("goghmode copy"));
}

#[test]
fn the_raycast_script_installs_under_application_support_by_default() {
    let home_dir = Path::new("/Users/example");

    assert_eq!(
        raycast::default_script_dir(home_dir),
        Path::new("/Users/example/Library/Application Support/GoghMode/raycast")
    );
}
