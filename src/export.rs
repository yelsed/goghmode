use std::collections::HashSet;
use std::fs;
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use image::{Rgba, RgbaImage};
use serde::Serialize;

use crate::drawing::{DrawingSnapshot, Narration, PageRef, Point, Ruling, RulingStyle, Stroke};
use crate::timeline::{self, Step, Window};

#[derive(Clone, Debug, PartialEq)]
pub struct ExportedFiles {
    pub json: PathBuf,
    pub svg: PathBuf,
    pub png: PathBuf,
    /// Present only for a narrated sheet: the markdown timeline and the
    /// directory of step crops beside it.
    pub timeline: Option<PathBuf>,
    pub steps: Option<PathBuf>,
    pub updated_at: u128,
}

#[derive(Serialize)]
struct ExportJson<'a> {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<&'a PageRef>,
    canvas: &'a crate::drawing::CanvasSize,
    strokes: &'a [Stroke],
    #[serde(skip_serializing_if = "Option::is_none")]
    narration: Option<&'a Narration>,
    #[serde(rename = "updatedAt")]
    updated_at: u128,
    files: ExportJsonFiles,
}

#[derive(Serialize)]
struct ExportJsonFiles {
    json: String,
    svg: String,
    png: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    steps: Option<String>,
}

const PAPER: Rgba<u8> = Rgba([255, 255, 255, 255]);

/// Under the ink added in a step. `stamp-review` from DESIGN.md, lightened
/// until it reads as marked paper rather than as ink, so the strokes on top of
/// it keep their own colour.
const HALO_INK: Rgba<u8> = Rgba([214, 228, 240, 255]);
/// How far the halo reaches past the stroke's own edge, in page units.
const HALO_EXTRA_RADIUS: f32 = 6.0;
/// Paper kept around a step's ink so the crop shows its surroundings.
const CROP_PADDING: f32 = 24.0;
/// A crop is scaled down until its long side fits this. Small on purpose: an
/// agent opens one per step, and each costs tokens.
const CROP_LONG_SIDE: f32 = 512.0;

pub fn snapshot_to_svg(snapshot: &DrawingSnapshot) -> String {
    let width = canvas_extent(snapshot.canvas.width);
    let height = canvas_extent(snapshot.canvas.height);
    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
        width, height, width, height
    ));
    svg.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\n");

    if let Some(ruling) = snapshot.canvas.ruling {
        push_ruling_svg(&mut svg, ruling, width as f32, height as f32);
    }

    for stroke in &snapshot.strokes {
        let points: Vec<&Point> = stroke
            .points
            .iter()
            .filter(|point| {
                in_bounds(
                    point.x,
                    point.y,
                    snapshot.canvas.width,
                    snapshot.canvas.height,
                )
            })
            .collect();
        match points.as_slice() {
            [] => {}
            [point] => {
                svg.push_str(&format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>\n",
                    svg_number(point.x),
                    svg_number(point.y),
                    svg_number(stroke.width / 2.0),
                    escape_svg_attr(&stroke.color)
                ));
            }
            [first, rest @ ..] => {
                let mut data = format!("M {} {}", svg_number(first.x), svg_number(first.y));
                for point in rest {
                    data.push_str(&format!(
                        " L {} {}",
                        svg_number(point.x),
                        svg_number(point.y)
                    ));
                }
                svg.push_str(&format!(
                    "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\n",
                    escape_svg_attr(&data),
                    escape_svg_attr(&stroke.color),
                    svg_number(stroke.width)
                ));
            }
        }
    }

    svg.push_str("</svg>\n");
    svg
}

pub fn snapshot_to_rgba(snapshot: &DrawingSnapshot) -> RgbaImage {
    let width = canvas_extent(snapshot.canvas.width);
    let height = canvas_extent(snapshot.canvas.height);
    let mut image = RgbaImage::from_pixel(width, height, PAPER);

    if let Some(ruling) = snapshot.canvas.ruling {
        paint_ruling(&mut image, ruling);
    }

    let whole_page = Window {
        x: 0.0,
        y: 0.0,
        width: snapshot.canvas.width,
        height: snapshot.canvas.height,
        scale: 1.0,
    };
    for stroke in &snapshot.strokes {
        paint_stroke(
            &mut image,
            stroke,
            snapshot,
            &whole_page,
            stroke.width / 2.0,
            parse_hex_rgb(&stroke.color),
        );
    }

    image
}

/// One step of a narrated sheet as the agent sees it: the window around the
/// ink added in that step, with that ink on a halo, and everything drawn up to
/// and including that step in its own colour. Strokes from later steps are not
/// there yet, because the crop is the sheet as it stood then.
pub fn render_step_crop(
    snapshot: &DrawingSnapshot,
    step: &Step,
    drawn_so_far: &[usize],
    window: &Window,
) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(window.pixel_width(), window.pixel_height(), PAPER);

    if let Some(ruling) = snapshot.canvas.ruling {
        paint_ruling_in_window(&mut image, ruling, snapshot, window);
    }

    let added_now: HashSet<usize> = step.strokes.iter().copied().collect();
    // The halo goes down first, under all ink, so earlier strokes that cross it
    // stay legible and the halo can never hide anything.
    for &index in &step.strokes {
        let stroke = &snapshot.strokes[index];
        paint_stroke(
            &mut image,
            stroke,
            snapshot,
            window,
            (stroke.width / 2.0 + HALO_EXTRA_RADIUS) * window.scale,
            HALO_INK,
        );
    }
    for &index in drawn_so_far
        .iter()
        .filter(|index| !added_now.contains(index))
    {
        let stroke = &snapshot.strokes[index];
        paint_stroke(
            &mut image,
            stroke,
            snapshot,
            window,
            stroke.width / 2.0 * window.scale,
            parse_hex_rgb(&stroke.color),
        );
    }
    for &index in &step.strokes {
        let stroke = &snapshot.strokes[index];
        paint_stroke(
            &mut image,
            stroke,
            snapshot,
            window,
            stroke.width / 2.0 * window.scale,
            parse_hex_rgb(&stroke.color),
        );
    }

    image
}

/// The window a step's crop shows: its ink, the halo around it, some paper,
/// clamped to the page and scaled to fit `CROP_LONG_SIDE`. `None` when the step
/// has no ink inside the page.
pub fn step_window(snapshot: &DrawingSnapshot, step: &Step) -> Option<Window> {
    let mut left = f32::INFINITY;
    let mut top = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    for &index in &step.strokes {
        let stroke = &snapshot.strokes[index];
        let reach = stroke.width / 2.0 + HALO_EXTRA_RADIUS;
        for point in stroke.points.iter().filter(|point| {
            in_bounds(
                point.x,
                point.y,
                snapshot.canvas.width,
                snapshot.canvas.height,
            )
        }) {
            left = left.min(point.x - reach);
            top = top.min(point.y - reach);
            right = right.max(point.x + reach);
            bottom = bottom.max(point.y + reach);
        }
    }
    if !left.is_finite() || !top.is_finite() {
        return None;
    }

    let x = (left - CROP_PADDING).max(0.0);
    let y = (top - CROP_PADDING).max(0.0);
    let width = ((right + CROP_PADDING).min(snapshot.canvas.width) - x).max(1.0);
    let height = ((bottom + CROP_PADDING).min(snapshot.canvas.height) - y).max(1.0);
    let scale = (CROP_LONG_SIDE / width.max(height)).min(1.0);
    Some(Window {
        x,
        y,
        width,
        height,
        scale,
    })
}

/// Writes the JSON, SVG and PNG for one snapshot into `directory` as
/// `<stem>.{json,svg,png}`, and for a narrated sheet also `<stem>.timeline.md`
/// and `<stem>.steps/NNN.png`. `link_prefix` is the project-relative directory
/// the `files` block in the JSON should point at, so a consumer reading the
/// JSON can find its siblings. `updated_at_override` keeps a mirrored copy
/// stamped with the same time as its original.
///
/// A sheet without narration removes any timeline and crops left by an earlier
/// write under the same stem: the words must never outlive the ink they were
/// spoken over.
pub fn write_artifacts(
    snapshot: &DrawingSnapshot,
    directory: impl AsRef<Path>,
    stem: &str,
    link_prefix: &str,
    updated_at_override: Option<u128>,
) -> anyhow::Result<ExportedFiles> {
    let directory = directory.as_ref();
    fs::create_dir_all(directory)?;

    let json_path = directory.join(format!("{stem}.json"));
    let svg_path = directory.join(format!("{stem}.svg"));
    let png_path = directory.join(format!("{stem}.png"));
    let timeline_path = directory.join(format!("{stem}.timeline.md"));
    let steps_path = directory.join(format!("{stem}.steps"));
    let json_tmp = directory.join(format!("{stem}.json.tmp"));
    let svg_tmp = directory.join(format!("{stem}.svg.tmp"));
    let png_tmp = directory.join(format!("{stem}.png.tmp"));
    let timeline_tmp = directory.join(format!("{stem}.timeline.md.tmp"));
    let steps_tmp = directory.join(format!("{stem}.steps.tmp"));
    let steps_old = directory.join(format!("{stem}.steps.old"));
    // Leftovers of a write that was interrupted between renames.
    remove_dir_if_present(&steps_tmp)?;
    remove_dir_if_present(&steps_old)?;

    let updated_at = match updated_at_override {
        Some(updated_at) => updated_at,
        None => SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    };
    let narration = snapshot
        .narration
        .as_ref()
        .filter(|narration| !narration.segments.is_empty());
    let export_json = ExportJson {
        schema_version: snapshot.schema_version,
        page: snapshot.page.as_ref(),
        canvas: &snapshot.canvas,
        strokes: &snapshot.strokes,
        narration,
        updated_at,
        files: ExportJsonFiles {
            json: format!("{link_prefix}{stem}.json"),
            svg: format!("{link_prefix}{stem}.svg"),
            png: format!("{link_prefix}{stem}.png"),
            timeline: narration.map(|_| format!("{link_prefix}{stem}.timeline.md")),
            steps: narration.map(|_| format!("{link_prefix}{stem}.steps/")),
        },
    };
    fs::write(&json_tmp, serde_json::to_string_pretty(&export_json)?)?;
    fs::write(&svg_tmp, snapshot_to_svg(snapshot))?;
    image::DynamicImage::ImageRgba8(snapshot_to_rgba(snapshot))
        .save_with_format(&png_tmp, image::ImageFormat::Png)?;
    if narration.is_some() {
        let steps = timeline::build_steps(snapshot, timeline::MAX_STEPS);
        let windows = write_step_crops(snapshot, &steps, &steps_tmp)?;
        fs::write(
            &timeline_tmp,
            timeline::markdown(snapshot, &steps, &windows, stem),
        )?;
    }

    fs::rename(&json_tmp, &json_path)?;
    fs::rename(&svg_tmp, &svg_path)?;
    fs::rename(&png_tmp, &png_path)?;
    if narration.is_some() {
        fs::rename(&timeline_tmp, &timeline_path)?;
        if steps_path.exists() {
            fs::rename(&steps_path, &steps_old)?;
        }
        fs::rename(&steps_tmp, &steps_path)?;
        remove_dir_if_present(&steps_old)?;
    } else {
        remove_file_if_present(&timeline_path)?;
        remove_dir_if_present(&steps_path)?;
    }

    Ok(ExportedFiles {
        json: json_path,
        svg: svg_path,
        png: png_path,
        timeline: narration.map(|_| timeline_path),
        steps: narration.map(|_| steps_path),
        updated_at,
    })
}

/// Renders every step's crop into `directory`, numbered from `001.png`, and
/// returns each step's window so the markdown can say where it sits. A step
/// with no ink gets no crop and a `None`.
fn write_step_crops(
    snapshot: &DrawingSnapshot,
    steps: &[Step],
    directory: &Path,
) -> anyhow::Result<Vec<Option<Window>>> {
    fs::create_dir_all(directory)?;
    let mut drawn_so_far: Vec<usize> = Vec::new();
    let mut windows = Vec::with_capacity(steps.len());
    for (index, step) in steps.iter().enumerate() {
        drawn_so_far.extend(step.strokes.iter().copied());
        let Some(window) = step_window(snapshot, step) else {
            windows.push(None);
            continue;
        };
        let crop = render_step_crop(snapshot, step, &drawn_so_far, &window);
        write_palette_png(&crop, &directory.join(format!("{:03}.png", index + 1)))?;
        windows.push(Some(window));
    }
    Ok(windows)
}

/// An 8-bit palette PNG: a crop is paper, ruling, halo and a few inks, so a
/// palette is a fraction of the size of the same pixels as RGBA. A crop that
/// somehow needs more than 256 colours is written as plain RGB instead.
pub fn write_palette_png(image: &RgbaImage, path: &Path) -> anyhow::Result<()> {
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut indices: Vec<u8> = Vec::with_capacity((image.width() * image.height()) as usize);
    for pixel in image.pixels() {
        let colour = [pixel[0], pixel[1], pixel[2]];
        let index = match palette.iter().position(|known| *known == colour) {
            Some(index) => index,
            None => {
                if palette.len() == 256 {
                    return Ok(image::DynamicImage::ImageRgba8(image.clone())
                        .to_rgb8()
                        .save_with_format(path, image::ImageFormat::Png)?);
                }
                palette.push(colour);
                palette.len() - 1
            }
        };
        indices.push(index as u8);
    }

    let file = BufWriter::new(fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, image.width(), image.height());
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_palette(palette.concat());
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&indices)?;
    writer.finish()?;
    Ok(())
}

fn remove_file_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn remove_dir_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Ruling ink is fixed here rather than sent by the client, so nothing on the
/// network can put arbitrary marks in the file the agent reads. The value is
/// `rule-hair` from DESIGN.md: visible enough to write against, faint enough to
/// stay under the ink.
const RULING_INK: Rgba<u8> = Rgba([201, 196, 187, 255]);

/// Derived rather than written twice, so the SVG and the PNG cannot disagree.
fn ruling_ink_hex() -> String {
    let Rgba([red, green, blue, _]) = RULING_INK;
    format!("#{red:02X}{green:02X}{blue:02X}")
}

/// Where the rules fall across one axis. The first rule is one space in, so the
/// page does not start on a line sitting against its own edge.
fn ruling_stops(extent: f32, spacing: f32) -> Vec<f32> {
    if !spacing.is_finite() || spacing < crate::drawing::MIN_RULING_SPACING {
        return Vec::new();
    }

    let mut stops = Vec::new();
    let mut at = spacing;
    while at < extent {
        stops.push(at);
        at += spacing;
    }
    stops
}

fn push_ruling_svg(svg: &mut String, ruling: Ruling, width: f32, height: f32) {
    let down = ruling_stops(height, ruling.spacing);
    let across = ruling_stops(width, ruling.spacing);

    match ruling.style {
        RulingStyle::Lines => {
            for y in down {
                push_ruling_line_svg(svg, 0.0, y, width, y);
            }
        }
        RulingStyle::Grid => {
            for y in down {
                push_ruling_line_svg(svg, 0.0, y, width, y);
            }
            for x in across {
                push_ruling_line_svg(svg, x, 0.0, x, height);
            }
        }
        RulingStyle::Dots => {
            for y in &down {
                for x in &across {
                    svg.push_str(&format!(
                        "<circle cx=\"{}\" cy=\"{}\" r=\"1\" fill=\"{}\"/>\n",
                        svg_number(*x),
                        svg_number(*y),
                        ruling_ink_hex()
                    ));
                }
            }
        }
    }
}

fn push_ruling_line_svg(svg: &mut String, x1: f32, y1: f32, x2: f32, y2: f32) {
    svg.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>\n",
        svg_number(x1),
        svg_number(y1),
        svg_number(x2),
        svg_number(y2),
        ruling_ink_hex()
    ));
}

fn paint_ruling(image: &mut RgbaImage, ruling: Ruling) {
    let width = image.width();
    let height = image.height();
    let down = ruling_stops(height as f32, ruling.spacing);
    let across = ruling_stops(width as f32, ruling.spacing);

    let paint_row = |y: f32, image: &mut RgbaImage| {
        let row = y.round() as u32;
        if row >= height {
            return;
        }
        for column in 0..width {
            image.put_pixel(column, row, RULING_INK);
        }
    };

    match ruling.style {
        RulingStyle::Lines => {
            for y in down {
                paint_row(y, image);
            }
        }
        RulingStyle::Grid => {
            for y in down {
                paint_row(y, image);
            }
            for x in across {
                let column = x.round() as u32;
                if column >= width {
                    continue;
                }
                for row in 0..height {
                    image.put_pixel(column, row, RULING_INK);
                }
            }
        }
        RulingStyle::Dots => {
            for y in &down {
                for x in &across {
                    fill_brush(image, x.round() as i32, y.round() as i32, 1.0, RULING_INK);
                }
            }
        }
    }
}

/// The page's ruling as seen through a crop's window: the same stops, placed
/// where the window puts them, so a crop rules exactly where the page does.
fn paint_ruling_in_window(
    image: &mut RgbaImage,
    ruling: Ruling,
    snapshot: &DrawingSnapshot,
    window: &Window,
) {
    let width = image.width();
    let height = image.height();
    let rows: Vec<i64> = ruling_stops(snapshot.canvas.height, ruling.spacing)
        .into_iter()
        .map(|y| ((y - window.y) * window.scale).round() as i64)
        .filter(|row| (0..height as i64).contains(row))
        .collect();
    let columns: Vec<i64> = ruling_stops(snapshot.canvas.width, ruling.spacing)
        .into_iter()
        .map(|x| ((x - window.x) * window.scale).round() as i64)
        .filter(|column| (0..width as i64).contains(column))
        .collect();

    match ruling.style {
        RulingStyle::Lines => {
            for row in rows {
                for column in 0..width {
                    image.put_pixel(column, row as u32, RULING_INK);
                }
            }
        }
        RulingStyle::Grid => {
            for &row in &rows {
                for column in 0..width {
                    image.put_pixel(column, row as u32, RULING_INK);
                }
            }
            for column in columns {
                for row in 0..height {
                    image.put_pixel(column as u32, row, RULING_INK);
                }
            }
        }
        RulingStyle::Dots => {
            for &row in &rows {
                for &column in &columns {
                    fill_brush(image, column as i32, row as i32, window.scale, RULING_INK);
                }
            }
        }
    }
}

/// One stroke onto an image, through a window. Points outside the page are
/// skipped, as the SVG skips them; a lone point is a dot.
fn paint_stroke(
    image: &mut RgbaImage,
    stroke: &Stroke,
    snapshot: &DrawingSnapshot,
    window: &Window,
    radius: f32,
    ink: Rgba<u8>,
) {
    let mut placed = stroke
        .points
        .iter()
        .filter(|point| {
            in_bounds(
                point.x,
                point.y,
                snapshot.canvas.width,
                snapshot.canvas.height,
            )
        })
        .map(|point| {
            (
                ((point.x - window.x) * window.scale).round() as i32,
                ((point.y - window.y) * window.scale).round() as i32,
            )
        });
    let Some(first) = placed.next() else {
        return;
    };
    fill_brush(image, first.0, first.1, radius, ink);
    let mut previous = first;
    for point in placed {
        draw_segment(image, previous, point, radius, ink);
        previous = point;
    }
}

fn canvas_extent(value: f32) -> u32 {
    if value.is_finite() {
        value.ceil().max(1.0) as u32
    } else {
        1
    }
}

fn in_bounds(x: f32, y: f32, width: f32, height: f32) -> bool {
    x.is_finite() && y.is_finite() && x >= 0.0 && y >= 0.0 && x <= width && y <= height
}

fn svg_number(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{:.0}", value)
    } else {
        let mut text = format!("{:.3}", value);
        while text.contains('.') && text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
        text
    }
}

fn escape_svg_attr(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

const DEFAULT_INK: Rgba<u8> = Rgba([17, 24, 39, 255]);

fn parse_hex_rgb(color: &str) -> Rgba<u8> {
    let digits = color.strip_prefix('#').unwrap_or(color);
    let expanded = match digits.len() {
        3 => digits.chars().flat_map(|digit| [digit, digit]).collect(),
        6 => digits.to_owned(),
        _ => return DEFAULT_INK,
    };

    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&expanded[range], 16).ok();
    match (channel(0..2), channel(2..4), channel(4..6)) {
        (Some(red), Some(green), Some(blue)) => Rgba([red, green, blue, 255]),
        _ => DEFAULT_INK,
    }
}

fn draw_segment(
    image: &mut RgbaImage,
    start: (i32, i32),
    end: (i32, i32),
    radius: f32,
    ink: Rgba<u8>,
) {
    let (mut x0, mut y0) = start;
    let (x1, y1) = end;

    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        fill_brush(image, x0, y0, radius, ink);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn fill_brush(image: &mut RgbaImage, cx: i32, cy: i32, radius: f32, ink: Rgba<u8>) {
    let radius = radius.max(0.5);
    let extent = radius.ceil() as i32;
    let radius_squared = radius * radius;
    for y in (cy - extent)..=(cy + extent) {
        for x in (cx - extent)..=(cx + extent) {
            let dx = x - cx;
            let dy = y - cy;
            let inside_brush = (dx * dx + dy * dy) as f32 <= radius_squared;
            let inside_image =
                x >= 0 && y >= 0 && (x as u32) < image.width() && (y as u32) < image.height();
            if inside_brush && inside_image {
                image.put_pixel(x as u32, y as u32, ink);
            }
        }
    }
}
