//! A narrated sheet read as steps: what was said, and the ink that was added
//! while it was said.
//!
//! The host only sorts. Every time here is unix milliseconds from the drawing
//! device's own clock, so a stroke's `started_at` and a spoken segment's
//! `start` are directly comparable.

use crate::drawing::DrawingSnapshot;

/// Above this, neighbouring steps are merged evenly. An agent reading the
/// timeline opens one crop per step, so hundreds of steps would cost more than
/// they tell.
pub const MAX_STEPS: usize = 200;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Step {
    /// Indices into `narration.segments`, in spoken order.
    pub segments: Vec<usize>,
    /// Indices into `snapshot.strokes`, in drawing order.
    pub strokes: Vec<usize>,
}

/// Where a step's crop sits on the page, in page units, and how far it was
/// scaled down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

impl Window {
    pub fn pixel_width(&self) -> u32 {
        (self.width * self.scale).ceil().max(1.0) as u32
    }

    pub fn pixel_height(&self) -> u32 {
        (self.height * self.scale).ceil().max(1.0) as u32
    }

    /// A coarse place name from the window's centre, so the agent can find the
    /// crop on the whole page without doing arithmetic.
    pub fn region(&self, canvas_width: f32, canvas_height: f32) -> &'static str {
        let column = third((self.x + self.width / 2.0) / canvas_width.max(1.0));
        let row = third((self.y + self.height / 2.0) / canvas_height.max(1.0));
        match (row, column) {
            (0, 0) => "top-left",
            (0, 1) => "top",
            (0, _) => "top-right",
            (1, 0) => "left",
            (1, 1) => "centre",
            (1, _) => "right",
            (_, 0) => "bottom-left",
            (_, 1) => "bottom",
            (_, _) => "bottom-right",
        }
    }
}

fn third(fraction: f32) -> u8 {
    if fraction < 1.0 / 3.0 {
        0
    } else if fraction < 2.0 / 3.0 {
        1
    } else {
        2
    }
}

/// One step per spoken segment, each holding the strokes begun while it was
/// the most recent thing said, plus a leading step for ink laid down before the
/// first word. Segments that added no ink fold into the step before them, so
/// every step has ink to show and every sentence is kept.
pub fn build_steps(snapshot: &DrawingSnapshot, max_steps: usize) -> Vec<Step> {
    let Some(narration) = snapshot.narration.as_ref() else {
        return Vec::new();
    };

    let mut segment_order: Vec<usize> = (0..narration.segments.len()).collect();
    segment_order.sort_by_key(|&index| narration.segments[index].start);
    let mut stroke_order: Vec<usize> = (0..snapshot.strokes.len()).collect();
    stroke_order.sort_by_key(|&index| snapshot.strokes[index].started_at.unwrap_or(0));

    let mut steps: Vec<Step> = Vec::with_capacity(segment_order.len() + 1);
    steps.push(Step::default());
    for &segment_index in &segment_order {
        steps.push(Step {
            segments: vec![segment_index],
            strokes: Vec::new(),
        });
    }
    for &stroke_index in &stroke_order {
        let started = snapshot.strokes[stroke_index].started_at.unwrap_or(0);
        let spoken_before = segment_order
            .iter()
            .take_while(|&&segment_index| narration.segments[segment_index].start <= started)
            .count();
        steps[spoken_before].strokes.push(stroke_index);
    }

    cap(fold_silent_steps(steps), max_steps)
}

fn fold_silent_steps(steps: Vec<Step>) -> Vec<Step> {
    let mut folded: Vec<Step> = Vec::new();
    // Words spoken before any ink at all wait for the first step that has some.
    let mut spoken_before_any_ink: Vec<usize> = Vec::new();
    for mut step in steps {
        if step.strokes.is_empty() {
            match folded.last_mut() {
                Some(previous) => previous.segments.extend(step.segments),
                None => spoken_before_any_ink.extend(step.segments),
            }
            continue;
        }
        if !spoken_before_any_ink.is_empty() {
            spoken_before_any_ink.append(&mut step.segments);
            step.segments = std::mem::take(&mut spoken_before_any_ink);
        }
        folded.push(step);
    }
    if folded.is_empty() && !spoken_before_any_ink.is_empty() {
        folded.push(Step {
            segments: spoken_before_any_ink,
            strokes: Vec::new(),
        });
    }
    folded
}

fn cap(steps: Vec<Step>, max_steps: usize) -> Vec<Step> {
    if steps.len() <= max_steps.max(1) {
        return steps;
    }
    let group_size = steps.len().div_ceil(max_steps.max(1));
    steps
        .chunks(group_size)
        .map(|group| Step {
            segments: group
                .iter()
                .flat_map(|step| step.segments.iter().copied())
                .collect(),
            strokes: group
                .iter()
                .flat_map(|step| step.strokes.iter().copied())
                .collect(),
        })
        .collect()
}

/// Earliest and latest moment in a step, from its words and its strokes.
pub fn step_span(snapshot: &DrawingSnapshot, step: &Step) -> Option<(u64, u64)> {
    let narration = snapshot.narration.as_ref()?;
    let mut start: Option<u64> = None;
    let mut end: Option<u64> = None;
    let mut widen = |from: u64, to: u64| {
        start = Some(start.map_or(from, |current| current.min(from)));
        end = Some(end.map_or(to, |current| current.max(to)));
    };
    for &segment_index in &step.segments {
        let segment = &narration.segments[segment_index];
        widen(segment.start, segment.end);
    }
    for &stroke_index in &step.strokes {
        if let Some(started) = snapshot.strokes[stroke_index].started_at {
            widen(started, started);
        }
    }
    Some((start?, end?))
}

/// The markdown an agent reads first. Crop paths are relative to the file, so
/// the same text serves `latest.*` and a page's own copy.
pub fn markdown(
    snapshot: &DrawingSnapshot,
    steps: &[Step],
    windows: &[Option<Window>],
    stem: &str,
) -> String {
    let Some(narration) = snapshot.narration.as_ref() else {
        return String::new();
    };
    let origin = steps
        .iter()
        .filter_map(|step| step_span(snapshot, step))
        .map(|(start, _)| start)
        .min()
        .unwrap_or(0);
    let close = steps
        .iter()
        .filter_map(|step| step_span(snapshot, step))
        .map(|(_, end)| end)
        .max()
        .unwrap_or(origin);
    let title = snapshot
        .page
        .as_ref()
        .and_then(|page| page.title.clone())
        .or_else(|| snapshot.page.as_ref().map(|page| page.id.clone()))
        .unwrap_or_else(|| "Sheet".to_owned());
    let canvas_width = snapshot.canvas.width;
    let canvas_height = snapshot.canvas.height;

    let mut text = String::new();
    text.push_str(&format!("# {title} — narrated timeline\n\n"));
    text.push_str(&format!(
        "Language: {} · {} steps · {} spoken segments · {} spoken and drawn · page {}×{}.\n\n",
        narration.language,
        steps.len(),
        narration.segments.len(),
        clock(close.saturating_sub(origin)),
        number(canvas_width),
        number(canvas_height)
    ));
    text.push_str(&format!(
        "The whole page is `{stem}.png`. Each step below names a crop: a window on that page, \
         scaled down, showing only where ink was added during the step. Ink added in the step \
         sits on a pale blue halo; every stroke keeps its own colour and width, and earlier ink \
         inside the window is drawn unchanged. Times are minutes:seconds from the first stroke \
         or word.\n"
    ));

    for (index, step) in steps.iter().enumerate() {
        let number_of_step = index + 1;
        let (start, end) = step_span(snapshot, step).unwrap_or((origin, origin));
        let window = windows.get(index).copied().flatten();
        text.push_str(&format!(
            "\n## Step {number_of_step} · {}–{}",
            clock(start.saturating_sub(origin)),
            clock(end.saturating_sub(origin))
        ));
        if window.is_some() {
            text.push_str(&format!(" · {stem}.steps/{number_of_step:03}.png"));
        }
        text.push('\n');

        if step.segments.is_empty() {
            text.push_str("> (nothing said)\n");
        }
        for &segment_index in &step.segments {
            let spoken = narration.segments[segment_index]
                .text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            text.push_str(&format!("> {spoken}\n"));
        }

        match window {
            Some(window) => text.push_str(&format!(
                "Drawn: {} {}, {} · window x {}–{}, y {}–{}.\n",
                step.strokes.len(),
                if step.strokes.len() == 1 {
                    "stroke"
                } else {
                    "strokes"
                },
                window.region(canvas_width, canvas_height),
                number(window.x),
                number(window.x + window.width),
                number(window.y),
                number(window.y + window.height)
            )),
            None => text.push_str("Drawn: nothing new.\n"),
        }
    }
    text
}

fn clock(milliseconds: u64) -> String {
    let seconds = milliseconds / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn number(value: f32) -> String {
    format!("{}", value.round() as i64)
}
