use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use image::{Rgba, RgbaImage};
use serde::Serialize;

use crate::drawing::{DrawingSnapshot, Point, Stroke};

#[derive(Clone, Debug, PartialEq)]
pub struct ExportedFiles {
    pub json: PathBuf,
    pub svg: PathBuf,
    pub png: PathBuf,
}

#[derive(Serialize)]
struct ExportJson<'a> {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    canvas: &'a crate::drawing::CanvasSize,
    strokes: &'a [Stroke],
    #[serde(rename = "updatedAt")]
    updated_at: u128,
    files: ExportJsonFiles,
}

#[derive(Serialize)]
struct ExportJsonFiles {
    json: &'static str,
    svg: &'static str,
    png: &'static str,
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
        fill_brush(
            &mut image,
            first.x.round() as i32,
            first.y.round() as i32,
            radius,
        );
        let mut previous = first;
        for point in points {
            draw_segment(&mut image, previous, point, radius);
            previous = point;
        }
    }

    image
}

pub fn write_snapshot(
    snapshot: &DrawingSnapshot,
    drawings_dir: impl AsRef<Path>,
) -> anyhow::Result<ExportedFiles> {
    let drawings_dir = drawings_dir.as_ref();
    fs::create_dir_all(drawings_dir)?;

    let json_path = drawings_dir.join("latest.json");
    let svg_path = drawings_dir.join("latest.svg");
    let png_path = drawings_dir.join("latest.png");
    let json_tmp = drawings_dir.join("latest.json.tmp");
    let svg_tmp = drawings_dir.join("latest.svg.tmp");
    let png_tmp = drawings_dir.join("latest.png.tmp");

    let updated_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let export_json = ExportJson {
        schema_version: snapshot.schema_version,
        canvas: &snapshot.canvas,
        strokes: &snapshot.strokes,
        updated_at,
        files: ExportJsonFiles {
            json: "drawings/latest.json",
            svg: "drawings/latest.svg",
            png: "drawings/latest.png",
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
    })
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

fn draw_segment(image: &mut RgbaImage, start: &Point, end: &Point, radius: f32) {
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
        fill_brush(image, x0, y0, radius);
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

fn fill_brush(image: &mut RgbaImage, cx: i32, cy: i32, radius: f32) {
    let radius = radius.max(0.5);
    let extent = radius.ceil() as i32;
    let radius_squared = radius * radius;
    for y in (cy - extent)..=(cy + extent) {
        for x in (cx - extent)..=(cx + extent) {
            let dx = x - cx;
            let dy = y - cy;
            if (dx * dx + dy * dy) as f32 <= radius_squared {
                if x >= 0 && y >= 0 && (x as u32) < image.width() && (y as u32) < image.height() {
                    image.put_pixel(x as u32, y as u32, Rgba([17, 24, 39, 255]));
                }
            }
        }
    }
}
