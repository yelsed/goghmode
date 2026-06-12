use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
    pub t: u128,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub id: String,
    pub color: String,
    pub width: f32,
    pub points: Vec<Point>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasSize {
    pub width: f32,
    pub height: f32,
    pub background: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawingSnapshot {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u8,
    pub canvas: CanvasSize,
    pub strokes: Vec<Stroke>,
}

pub struct Drawing {
    canvas: CanvasSize,
    strokes: Vec<Stroke>,
    active: Option<Stroke>,
    next_id: u64,
    color: String,
    width: f32,
}

impl Drawing {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            canvas: CanvasSize {
                width: width.max(1.0),
                height: height.max(1.0),
                background: "#ffffff".to_owned(),
            },
            strokes: Vec::new(),
            active: None,
            next_id: 1,
            color: "#111827".to_owned(),
            width: 4.0,
        }
    }

    pub fn set_canvas_size(&mut self, width: f32, height: f32) {
        self.canvas.width = if width.is_finite() {
            width.max(1.0)
        } else {
            1.0
        };
        self.canvas.height = if height.is_finite() {
            height.max(1.0)
        } else {
            1.0
        };
    }

    pub fn set_brush_width(&mut self, width: f32) {
        if width.is_finite() {
            self.width = width.clamp(1.0, 80.0);
        }
    }

    pub fn begin_stroke(&mut self, x: f32, y: f32, pressure: f32, now_ms: u128) {
        if !self.accepts_point(x, y, pressure) {
            return;
        }

        let point = Point {
            x,
            y,
            pressure: pressure.clamp(0.0, 1.0),
            t: now_ms,
        };
        let stroke = Stroke {
            id: format!("stroke-{}", self.next_id),
            color: self.color.clone(),
            width: self.width,
            points: vec![point],
        };
        self.next_id += 1;
        self.active = Some(stroke);
    }

    pub fn push_point(&mut self, x: f32, y: f32, pressure: f32, now_ms: u128) {
        if !self.accepts_point(x, y, pressure) {
            return;
        }
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if active.points.last().is_some_and(|last| {
            last.x == x && last.y == y && last.pressure == pressure.clamp(0.0, 1.0)
        }) {
            return;
        }
        active.points.push(Point {
            x,
            y,
            pressure: pressure.clamp(0.0, 1.0),
            t: now_ms,
        });
    }

    pub fn finish_stroke(&mut self) {
        if let Some(active) = self.active.take() {
            if !active.points.is_empty() {
                self.strokes.push(active);
            }
        }
    }

    pub fn undo(&mut self) {
        self.strokes.pop();
    }

    pub fn clear(&mut self) {
        self.strokes.clear();
        self.active = None;
    }

    pub fn snapshot(&self) -> DrawingSnapshot {
        let mut strokes = self.strokes.clone();
        if let Some(active) = &self.active {
            if !active.points.is_empty() {
                strokes.push(active.clone());
            }
        }

        DrawingSnapshot {
            schema_version: 1,
            canvas: self.canvas.clone(),
            strokes,
        }
    }

    pub fn strokes(&self) -> &[Stroke] {
        &self.strokes
    }

    pub fn active_stroke(&self) -> Option<&Stroke> {
        self.active.as_ref()
    }

    pub fn brush_width(&self) -> f32 {
        self.width
    }

    fn accepts_point(&self, x: f32, y: f32, pressure: f32) -> bool {
        x.is_finite()
            && y.is_finite()
            && pressure.is_finite()
            && x >= 0.0
            && y >= 0.0
            && x <= self.canvas.width
            && y <= self.canvas.height
    }
}
