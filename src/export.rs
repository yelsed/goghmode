use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use image::{Rgba, RgbaImage};
use serde::Serialize;

use crate::drawing::{DrawingSnapshot, PageRef, Point, Ruling, RulingStyle, Stroke};

#[derive(Clone, Debug, PartialEq)]
pub struct ExportedFiles {
    pub json: PathBuf,
    pub svg: PathBuf,
    pub png: PathBuf,
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
    #[serde(rename = "updatedAt")]
    updated_at: u128,
    files: ExportJsonFiles,
}

#[derive(Serialize)]
struct ExportJsonFiles {
    json: String,
    svg: String,
    png: String,
}

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
    let mut image = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));

    if let Some(ruling) = snapshot.canvas.ruling {
        paint_ruling(&mut image, ruling);
    }

    for stroke in &snapshot.strokes {
        let mut points = stroke.points.iter().filter(|point| {
            in_bounds(
                point.x,
                point.y,
                snapshot.canvas.width,
                snapshot.canvas.height,
            )
        });
        let Some(first) = points.next() else {
            continue;
        };
        let radius = stroke.width / 2.0;
        let ink = parse_hex_rgb(&stroke.color);
        fill_brush(
            &mut image,
            first.x.round() as i32,
            first.y.round() as i32,
            radius,
            ink,
        );
        let mut previous = first;
        for point in points {
            draw_segment(&mut image, previous, point, radius, ink);
            previous = point;
        }
    }

    image
}

/// Writes the JSON, SVG and PNG for one snapshot into `directory` as
/// `<stem>.{json,svg,png}`. `link_prefix` is the project-relative directory the
/// `files` block in the JSON should point at, so a consumer reading the JSON can
/// find its siblings. `updated_at_override` keeps a mirrored copy stamped with
/// the same time as its original.
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
    let json_tmp = directory.join(format!("{stem}.json.tmp"));
    let svg_tmp = directory.join(format!("{stem}.svg.tmp"));
    let png_tmp = directory.join(format!("{stem}.png.tmp"));

    let updated_at = match updated_at_override {
        Some(updated_at) => updated_at,
        None => SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    };
    let export_json = ExportJson {
        schema_version: snapshot.schema_version,
        page: snapshot.page.as_ref(),
        canvas: &snapshot.canvas,
        strokes: &snapshot.strokes,
        updated_at,
        files: ExportJsonFiles {
            json: format!("{link_prefix}{stem}.json"),
            svg: format!("{link_prefix}{stem}.svg"),
            png: format!("{link_prefix}{stem}.png"),
        },
    };
    fs::write(&json_tmp, serde_json::to_string_pretty(&export_json)?)?;
    fs::write(&svg_tmp, snapshot_to_svg(snapshot))?;
    image::DynamicImage::ImageRgba8(snapshot_to_rgba(snapshot))
        .save_with_format(&png_tmp, image::ImageFormat::Png)?;

    fs::rename(&json_tmp, &json_path)?;
    fs::rename(&svg_tmp, &svg_path)?;
    fs::rename(&png_tmp, &png_path)?;

    Ok(ExportedFiles {
        json: json_path,
        svg: svg_path,
        png: png_path,
        updated_at,
    })
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

fn draw_segment(image: &mut RgbaImage, start: &Point, end: &Point, radius: f32, ink: Rgba<u8>) {
    let mut x0 = start.x.round() as i32;
    let mut y0 = start.y.round() as i32;
    let x1 = end.x.round() as i32;
    let y1 = end.y.round() as i32;

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
