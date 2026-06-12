use std::borrow::Cow;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use arboard::{Clipboard, ImageData};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke as EguiStroke, Vec2};

use crate::drawing::{Drawing, Stroke};
use crate::export::{snapshot_to_rgba, write_snapshot};
use crate::mobile_server::MobileServer;
use crate::prompt::{prompt_text, PromptTarget};

pub struct GoghModeApp {
    drawing: Drawing,
    drawings_dir: PathBuf,
    mobile_server: Option<MobileServer>,
    status: String,
}

impl GoghModeApp {
    pub fn new(drawings_dir: PathBuf) -> Self {
        let mobile_server = MobileServer::start(&drawings_dir).ok();
        let status = match mobile_server.as_ref() {
            Some(server) => format!(
                "Draw, then Copy image, Copy prompt, or use /goghmode. Mobile: {}",
                server.url()
            ),
            None => {
                "Draw, then Copy image, Copy prompt, or use /goghmode. Mobile server unavailable."
                    .to_owned()
            }
        };

        Self {
            drawing: Drawing::new(1024.0, 640.0),
            drawings_dir,
            mobile_server,
            status,
        }
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        self.save_with_status("Saved drawings/latest.svg")
    }

    pub fn copy_prompt(&mut self) {
        match Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(prompt_text(PromptTarget::Generic)))
        {
            Ok(()) => self.status = "Copied AI prompt to clipboard".to_owned(),
            Err(_) => {
                self.status =
                    "Clipboard unavailable; use Print prompt or goghmode prompt".to_owned()
            }
        }
    }

    pub fn copy_image(&mut self) {
        let image = snapshot_to_rgba(&self.drawing.snapshot());
        let width = image.width() as usize;
        let height = image.height() as usize;
        let bytes = image.into_raw();
        let data = ImageData {
            width,
            height,
            bytes: Cow::Owned(bytes),
        };

        match Clipboard::new().and_then(|mut clipboard| clipboard.set_image(data)) {
            Ok(()) => self.status = "Copied drawing image to clipboard".to_owned(),
            Err(_) => {
                self.status = "Image clipboard unavailable; use Copy prompt or /goghmode".to_owned()
            }
        }
    }

    pub fn copy_mobile_url(&mut self) {
        let Some(server) = &self.mobile_server else {
            self.status = "Mobile server unavailable".to_owned();
            return;
        };

        match Clipboard::new().and_then(|mut clipboard| clipboard.set_text(server.url())) {
            Ok(()) => self.status = "Copied mobile URL to clipboard".to_owned(),
            Err(_) => self.status = "Clipboard unavailable; type the Mobile URL".to_owned(),
        }
    }

    pub fn print_prompt(&mut self) {
        println!("{}", prompt_text(PromptTarget::Generic));
        self.status = "Printed AI prompt to terminal".to_owned();
    }

    fn save_with_status(&mut self, success_status: &str) -> anyhow::Result<()> {
        let result = write_snapshot(&self.drawing.snapshot(), &self.drawings_dir).map(|_| ());
        match result {
            Ok(()) => {
                self.status = success_status.to_owned();
                Ok(())
            }
            Err(error) => {
                self.status = format!("Save failed: {error}");
                Err(error)
            }
        }
    }

    fn save_after_undo(&mut self) {
        let _ = self.save_with_status("Undid last stroke and saved drawings/latest.svg");
    }

    fn save_after_clear(&mut self) {
        let _ = self.save_with_status("Cleared canvas and saved drawings/latest.svg");
    }
}

impl eframe::App for GoghModeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        configure_visuals(ui.ctx());
        ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);

        self.draw_toolbar(ui);
        ui.add_space(10.0);
        self.draw_canvas(ui);
        ui.add_space(8.0);
        StatusBar::show(ui, &self.status);
    }
}

impl GoghModeApp {
    fn draw_toolbar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(Color32::from_rgb(18, 24, 34))
            .stroke(EguiStroke::new(1.0, Color32::from_rgb(42, 53, 68)))
            .corner_radius(egui::CornerRadius::same(16))
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("GoghMode")
                                .size(24.0)
                                .strong()
                                .color(Color32::from_rgb(245, 247, 250)),
                        );
                        ui.label(
                            RichText::new("Local sketchpad for Claude")
                                .size(13.0)
                                .color(Color32::from_rgb(165, 176, 192)),
                        );
                    });

                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        let mut brush_width = self.drawing.brush_width();
                        ui.add_sized(
                            [190.0, 30.0],
                            egui::Slider::new(&mut brush_width, 1.0..=32.0)
                                .text("Brush")
                                .step_by(1.0),
                        );
                        if brush_width != self.drawing.brush_width() {
                            self.drawing.set_brush_width(brush_width);
                        }

                        ui.separator();

                        if primary_button(ui, "Save").clicked() {
                            let _ = self.save();
                        }
                        if ui.button("Undo").clicked() {
                            self.drawing.undo();
                            self.save_after_undo();
                        }
                        if ui.button("Clear").clicked() {
                            self.drawing.clear();
                            self.save_after_clear();
                        }

                        ui.separator();

                        if primary_button(ui, "Send to Claude").clicked() {
                            self.copy_prompt();
                        }
                        if ui.button("Copy image").clicked() {
                            self.copy_image();
                        }
                        if ui.button("Print prompt").clicked() {
                            self.print_prompt();
                        }
                    });

                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Copy mobile URL").clicked() {
                            self.copy_mobile_url();
                        }
                        if let Some(server) = &self.mobile_server {
                            ui.label(
                                RichText::new("Mobile")
                                    .strong()
                                    .color(Color32::from_rgb(214, 220, 230)),
                            );
                            ui.monospace(server.url());
                        } else {
                            ui.label(
                                RichText::new("Mobile server unavailable")
                                    .color(Color32::from_rgb(244, 170, 170)),
                            );
                        }
                    });
                });
            });
    }

    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let size = Vec2::new(available.x.max(1.0), (available.y - 42.0).max(1.0));
        egui::Frame::canvas(ui.style())
            .fill(Color32::from_rgb(250, 249, 244))
            .stroke(EguiStroke::new(1.0, Color32::from_rgb(201, 206, 214)))
            .corner_radius(egui::CornerRadius::same(18))
            .inner_margin(egui::Margin::same(0))
            .show(ui, |ui| {
                let (canvas_rect, response) = ui.allocate_exact_size(size, Sense::drag());
                self.drawing
                    .set_canvas_size(canvas_rect.width(), canvas_rect.height());

                let painter = ui.painter_at(canvas_rect);
                painter.rect_filled(
                    canvas_rect,
                    egui::CornerRadius::same(18),
                    Color32::from_rgb(250, 249, 244),
                );
                painter.rect_stroke(
                    canvas_rect,
                    egui::CornerRadius::same(18),
                    EguiStroke::new(1.0, Color32::from_rgb(201, 206, 214)),
                    egui::StrokeKind::Inside,
                );

                if response.drag_started() {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        if let Some(local) = local_point(canvas_rect, pointer) {
                            self.drawing
                                .begin_stroke(local.x, local.y, 0.5, unix_millis());
                        }
                    }
                }

                if response.dragged() {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        if let Some(local) = local_point(canvas_rect, pointer) {
                            self.drawing
                                .push_point(local.x, local.y, 0.5, unix_millis());
                        }
                    }
                    ui.ctx().request_repaint();
                }

                if response.drag_stopped() {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        if let Some(local) = local_point(canvas_rect, pointer) {
                            self.drawing
                                .push_point(local.x, local.y, 0.5, unix_millis());
                        }
                    }
                    self.drawing.finish_stroke();
                    let _ = self.save();
                }

                for stroke in self.drawing.strokes() {
                    paint_stroke(&painter, canvas_rect, stroke);
                }
                if let Some(stroke) = self.drawing.active_stroke() {
                    paint_stroke(&painter, canvas_rect, stroke);
                }
            });
    }
}

struct StatusBar;

impl StatusBar {
    fn show(ui: &mut egui::Ui, status: &str) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(status)
                    .size(13.0)
                    .color(Color32::from_rgb(173, 184, 199)),
            );
        });
    }
}

fn configure_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = Color32::from_rgb(10, 14, 20);
    visuals.panel_fill = Color32::from_rgb(10, 14, 20);
    visuals.extreme_bg_color = Color32::from_rgb(10, 14, 20);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(32, 40, 52);
    visuals.widgets.inactive.fg_stroke.color = Color32::from_rgb(235, 239, 245);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(43, 54, 70);
    visuals.widgets.active.bg_fill = Color32::from_rgb(62, 76, 96);
    visuals.selection.bg_fill = Color32::from_rgb(203, 163, 70);
    visuals.selection.stroke.color = Color32::from_rgb(17, 24, 39);
    ctx.set_visuals(visuals);
}

fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .strong()
                .color(Color32::from_rgb(25, 29, 37)),
        )
        .fill(Color32::from_rgb(226, 183, 80))
        .stroke(EguiStroke::new(1.0, Color32::from_rgb(241, 205, 111))),
    )
}

fn local_point(canvas_rect: Rect, pointer: Pos2) -> Option<Pos2> {
    if canvas_rect.contains(pointer) {
        Some(Pos2::new(
            pointer.x - canvas_rect.left(),
            pointer.y - canvas_rect.top(),
        ))
    } else {
        None
    }
}

fn paint_stroke(painter: &egui::Painter, canvas_rect: Rect, stroke: &Stroke) {
    match stroke.points.as_slice() {
        [] => {}
        [point] => {
            painter.circle_filled(
                Pos2::new(canvas_rect.left() + point.x, canvas_rect.top() + point.y),
                stroke.width / 2.0,
                Color32::from_rgb(17, 24, 39),
            );
        }
        points => {
            let screen_points: Vec<Pos2> = points
                .iter()
                .map(|point| Pos2::new(canvas_rect.left() + point.x, canvas_rect.top() + point.y))
                .collect();
            painter.line(
                screen_points,
                EguiStroke::new(stroke.width, Color32::from_rgb(17, 24, 39)),
            );
        }
    }
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
