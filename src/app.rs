use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arboard::Clipboard;
use egui::{Color32, RichText, Sense, Stroke as EguiStroke, TextureHandle, Vec2};

use crate::host::{Host, PairingPayload, PairingState};
use crate::mobile_server::MobileServer;
use crate::pages::{
    list_pages, pages_dir, promote_page, read_pin, set_pin, sheet_numbers, PageEntry,
};

const THUMBNAIL_WIDTH: u32 = 240;
const THUMBNAIL_HEIGHT: u32 = 160;

/// macOS ships `open`; Linux desktops ship `xdg-open`. This is the only place
/// in the running application that has to know which platform it is on.
const FILE_MANAGER_COMMAND: &str = if cfg!(target_os = "macos") {
    "open"
} else {
    "xdg-open"
};

/// The Drawing Set world, as the desktop renders it. Kept in step with
/// DESIGN.md and the iPad's `Sheet` tokens — the two surfaces are one set, so
/// neither may carry a colour the other does not know about.
///
/// Resolved per call rather than held as constants because the window follows
/// the system appearance: paper in the light, and DESIGN.md's dark pairs
/// otherwise. A register that stays bright white inside a dark desktop is the
/// same "looks like two apps" problem in the other direction.
struct Sheet {
    ground: Color32,
    paper: Color32,
    edge: Color32,
    rule: Color32,
    ink: Color32,
    ink_label: Color32,
    stamp: Color32,
}

const LIGHT: Sheet = Sheet {
    ground: Color32::from_rgb(237, 234, 228),
    paper: Color32::from_rgb(255, 255, 255),
    edge: Color32::from_rgb(216, 212, 204),
    rule: Color32::from_rgb(168, 162, 154),
    ink: Color32::from_rgb(26, 25, 23),
    ink_label: Color32::from_rgb(107, 102, 94),
    stamp: Color32::from_rgb(180, 51, 31),
};

const DARK: Sheet = Sheet {
    ground: Color32::from_rgb(14, 13, 12),
    paper: Color32::from_rgb(28, 27, 25),
    edge: Color32::from_rgb(58, 55, 51),
    rule: Color32::from_rgb(90, 86, 80),
    ink: Color32::from_rgb(242, 239, 233),
    ink_label: Color32::from_rgb(142, 136, 128),
    // The stamp is the one colour that does not change. It is the mark, and a
    // mark that shifts with the appearance is not a mark.
    stamp: Color32::from_rgb(180, 51, 31),
};

fn sheet(ui: &egui::Ui) -> &'static Sheet {
    if ui.ctx().theme() == egui::Theme::Dark {
        &DARK
    } else {
        &LIGHT
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum View {
    Register,
    Devices,
}

/// The desktop's one job. Either it is serving devices, or it can say exactly
/// why it is not — there is no third state where it half works.
pub enum Bridge {
    Serving(MobileServer),
    Unavailable(String),
}

pub struct GoghModeApp {
    drawings_dir: PathBuf,
    bridge: Bridge,
    status: String,
    view: View,
    pages: Vec<PageEntry>,
    pinned_page_id: Option<String>,
    thumbnails: HashMap<String, TextureHandle>,
    host: Host,
    goghmode_dir: PathBuf,
    pairing_payload: Option<PairingPayload>,
    pairing_qr: Option<TextureHandle>,
    host_name_draft: String,
}

impl GoghModeApp {
    pub fn new(
        drawings_dir: PathBuf,
        host: Host,
        goghmode_dir: PathBuf,
        bridge: Bridge,
    ) -> Self {
        let status = match &bridge {
            Bridge::Serving(_) => "Draw on a paired device. The stamped sheet is what /goghmode reads.".to_owned(),
            Bridge::Unavailable(reason) => reason.clone(),
        };

        Self {
            pages: list_pages(&drawings_dir),
            pinned_page_id: read_pin(&drawings_dir),
            drawings_dir,
            bridge,
            status,
            view: View::Register,
            thumbnails: HashMap::new(),
            host_name_draft: host.identity().display_name,
            host,
            goghmode_dir,
            pairing_payload: None,
            pairing_qr: None,
        }
    }

    fn server(&self) -> Option<&MobileServer> {
        match &self.bridge {
            Bridge::Serving(server) => Some(server),
            Bridge::Unavailable(_) => None,
        }
    }

    fn refresh_pages(&mut self) {
        self.pages = list_pages(&self.drawings_dir);
        self.pinned_page_id = read_pin(&self.drawings_dir);
    }

    fn page_title(&self, page_id: &str) -> String {
        self.pages
            .iter()
            .find(|page| page.page_id == page_id)
            .and_then(|page| page.title.clone())
            .unwrap_or_else(|| page_id.to_owned())
    }

    /// Sends one sheet now without moving the stamp.
    fn send_page(&mut self, page_id: &str) {
        self.status = match promote_page(&self.drawings_dir, page_id) {
            // Only latest.* moves. The page keeps its own files and its place in
            // the register, so sending is not an edit.
            Ok(_) => format!("Sent {} to drawings/latest.*", self.page_title(page_id)),
            Err(error) => format!("Could not send that sheet: {error}"),
        };
    }

    /// Stamps a sheet as the one the agent reads, or lifts the stamp.
    fn stamp_page(&mut self, page_id: Option<&str>) {
        match set_pin(&self.drawings_dir, page_id) {
            Ok(()) => {
                self.pinned_page_id = page_id.map(str::to_owned);
                self.status = match page_id {
                    Some(page_id) => format!(
                        "Stamped {} — /goghmode reads it until you stamp another",
                        self.page_title(page_id)
                    ),
                    None => {
                        "Stamp lifted. latest.* follows whichever sheet is drawn on next".to_owned()
                    }
                };
            }
            Err(error) => self.status = format!("Could not stamp that sheet: {error}"),
        }
        self.refresh_pages();
    }

    fn reveal_drawings_dir(&mut self) {
        let target = &self.drawings_dir;
        let _ = std::fs::create_dir_all(target);
        self.status = match std::process::Command::new(FILE_MANAGER_COMMAND)
            .arg(target)
            .spawn()
        {
            Ok(_) => format!("Opened {}", target.display()),
            // Naming the command matters on Linux, where `xdg-open` comes from a
            // package that can genuinely be missing rather than being guaranteed.
            Err(error) => {
                format!("Could not open the drawings folder with {FILE_MANAGER_COMMAND}: {error}")
            }
        };
    }

    fn thumbnail_for(&mut self, ctx: &egui::Context, page: &PageEntry) -> Option<TextureHandle> {
        let key = format!("{}@{}", page.page_id, page.updated_at);
        if let Some(handle) = self.thumbnails.get(&key) {
            return Some(handle.clone());
        }

        let path = pages_dir(&self.drawings_dir)
            .join(&page.page_id)
            .join("page.png");
        let image = image::open(path).ok()?.to_rgba8();
        let thumbnail = image::imageops::thumbnail(&image, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT);
        let size = [thumbnail.width() as usize, thumbnail.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, thumbnail.as_raw());
        let handle = ctx.load_texture(&key, color_image, egui::TextureOptions::LINEAR);

        // Keyed by page id and stamp together, so an edited page reloads while
        // untouched ones stay cached.
        self.thumbnails
            .retain(|cached, _| !cached.starts_with(&format!("{}@", page.page_id)));
        self.thumbnails.insert(key, handle.clone());
        Some(handle)
    }

}

impl eframe::App for GoghModeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);

        self.draw_header(ui);
        ui.add_space(10.0);
        match self.view {
            // The register is paper on a paper-coloured ground, so sheets read
            // as objects lying on it rather than panels in an app.
            View::Register => {
                egui::Frame::new()
                    .fill(sheet(ui).ground)
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| self.draw_page_browser(ui));
            }
            View::Devices => self.draw_devices(ui),
        }
        ui.add_space(8.0);
        self.draw_footer(ui);
        ui.add_space(6.0);
        StatusBar::show(ui, &self.status);
    }
}

impl GoghModeApp {
    /// A title block on paper, the way DESIGN.md draws one, rather than loose
    /// text on the ground. Connection and Devices sit together at the right:
    /// they are one job — getting a device talking to this host — and putting
    /// Devices at the far bottom of the window made it unfindable.
    fn draw_header(&mut self, ui: &mut egui::Ui) {
        let sheet = sheet(ui);
        egui::Frame::new()
            .fill(sheet.paper)
            .stroke(EguiStroke::new(1.0, sheet.edge))
            .corner_radius(egui::CornerRadius::same(2))
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("GoghMode").size(20.0).strong().color(sheet.ink));
                    ui.label(
                        RichText::new("the drawing set your agent reads")
                            .size(12.0)
                            .color(sheet.ink_label),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let device_count = self.host.devices().len();
                        let (label, target) = match self.view {
                            View::Register => {
                                (format!("Devices ({device_count})"), View::Devices)
                            }
                            View::Devices => ("Back to register".to_owned(), View::Register),
                        };
                        if ui.button(label).clicked() {
                            self.view = target;
                            if target == View::Register {
                                self.refresh_pages();
                            }
                        }
                        self.draw_connection_chip(ui);
                    });
                });
            });
    }

    /// One chip in place of a whole row of furniture: a copy button, the word
    /// "Mobile", the URL in monospace, and a port warning were all permanently
    /// on screen for a fact you need about twice.
    ///
    /// egui has no popover, and a collapsing section would shove the register
    /// down every time it opened, so the detail lives in a menu.
    fn draw_connection_chip(&mut self, ui: &mut egui::Ui) {
        // Named, not merely dotted. "\u{25cf} Desley's MacBook" reads as a status
        // line you cannot press; "Connection" reads as somewhere to go, which is
        // what someone hunting for the mobile URL is actually looking for.
        let (dot, label) = match &self.bridge {
            Bridge::Serving(_) => (
                "\u{25cf}",
                format!("Connection \u{b7} {}", self.host.identity().display_name),
            ),
            Bridge::Unavailable(_) => ("\u{25cb}", "Connection \u{b7} not serving".to_owned()),
        };
        let tint = match &self.bridge {
            Bridge::Serving(_) => sheet(ui).ink,
            Bridge::Unavailable(_) => sheet(ui).stamp,
        };

        ui.menu_button(
            RichText::new(format!("{dot}  {label}  \u{25be}")).color(tint),
            |ui| {
            match &self.bridge {
                Bridge::Serving(server) => {
                    let url = server.url().to_owned();
                    ui.label(RichText::new("Mobile URL").size(11.0).color(sheet(ui).ink_label));
                    ui.monospace(&url);
                    ui.add_space(6.0);
                    if ui.button("Copy mobile URL").clicked() {
                        self.status = match Clipboard::new()
                            .and_then(|mut clipboard| clipboard.set_text(url))
                        {
                            Ok(()) => "Copied the mobile URL".to_owned(),
                            Err(_) => "Clipboard unavailable; read the URL above".to_owned(),
                        };
                        ui.close();
                    }
                }
                Bridge::Unavailable(reason) => {
                    ui.label(RichText::new(reason.clone()).color(sheet(ui).stamp));
                }
            }
        });
    }

    /// Where you go, rather than what you do — there is nothing left to do to a
    /// drawing from here.
    fn draw_footer(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let device_count = self.host.devices().len();
            let (label, target) = match self.view {
                View::Register => (format!("Devices ({device_count})"), View::Devices),
                View::Devices => ("Register".to_owned(), View::Register),
            };
            if ui.button(label).clicked() {
                self.view = target;
                if target == View::Register {
                    self.refresh_pages();
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Reveal drawings folder").clicked() {
                    self.reveal_drawings_dir();
                }
            });
        });
    }


    fn draw_devices(&mut self, ui: &mut egui::Ui) {
        let pairing = self.host.pairing_state();
        // Both the countdown and a request arriving from the network need the
        // panel to redraw without anyone touching the keyboard.
        if !matches!(pairing, PairingState::Idle) {
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }

        self.draw_host_identity(ui);
        ui.add_space(10.0);

        match &pairing {
            PairingState::Pending { request, .. } => {
                let request = request.clone();
                self.draw_approval_request(ui, &request);
            }
            PairingState::Armed { expires_at, .. } => {
                let remaining = expires_at.saturating_duration_since(std::time::Instant::now());
                self.draw_armed_pairing(ui, remaining);
            }
            PairingState::Idle => self.draw_pairing_start(ui),
        }

        ui.add_space(14.0);
        self.draw_device_list(ui);
        ui.add_space(14.0);
        self.draw_legacy_toggle(ui);
    }

    fn draw_host_identity(&mut self, ui: &mut egui::Ui) {
        let identity = self.host.identity();
        ui.horizontal(|ui| {
            ui.label(RichText::new("This host").strong());
            ui.add(egui::TextEdit::singleline(&mut self.host_name_draft).desired_width(240.0));
            if ui.button("Rename").clicked() {
                let goghmode_dir = self.goghmode_dir.clone();
                self.status = match self.host.set_display_name(&self.host_name_draft, &goghmode_dir)
                {
                    Ok(()) => format!("This host is now called {}", self.host_name_draft),
                    Err(error) => format!("Could not rename this host: {error}"),
                };
            }
        });
        // Shown so two hosts can be told apart at a glance when their names are
        // similar. It is not a secret — it is only useful to a paired device.
        ui.label(
            RichText::new(format!("Identity {}", short_host_id(&identity.host_id)))
                .size(12.0)
                .color(Color32::from_rgb(150, 162, 178)),
        );
    }

    fn draw_pairing_start(&mut self, ui: &mut egui::Ui) {
        if ui.button("Pair a device").clicked() {
            self.start_pairing(ui.ctx());
        }
        ui.label(
            RichText::new(
                "Pairing shows a code for two minutes. The device derives its own key from it, \
                 so nothing secret is ever sent over the network.",
            )
            .size(12.0)
            .color(Color32::from_rgb(150, 162, 178)),
        );
    }

    fn draw_armed_pairing(&mut self, ui: &mut egui::Ui, remaining: Duration) {
        ui.label(
            RichText::new(format!(
                "Scan this in the GoghMode companion — {} seconds left",
                remaining.as_secs()
            ))
            .strong(),
        );
        if let Some(texture) = &self.pairing_qr {
            ui.image((texture.id(), texture.size_vec2()));
        }
        if let Some(payload) = &self.pairing_payload {
            let text = serde_json::to_string(payload).unwrap_or_default();
            ui.label(
                RichText::new("Or paste this into the companion by hand")
                    .size(12.0)
                    .color(Color32::from_rgb(150, 162, 178)),
            );
            ui.add(egui::TextEdit::multiline(&mut text.as_str()).desired_rows(3));
            if ui.button("Copy pairing code").clicked() {
                let copied = Clipboard::new()
                    .and_then(|mut clipboard| clipboard.set_text(text.clone()))
                    .is_ok();
                self.status = if copied {
                    "Copied the pairing code".to_owned()
                } else {
                    "Clipboard unavailable; type the code shown".to_owned()
                };
            }
        }
        if ui.button("Cancel").clicked() {
            self.host.cancel_pairing();
            self.pairing_payload = None;
            self.pairing_qr = None;
        }
    }

    /// The approval sheet. A request only reaches here once it has proved it
    /// holds the shown code, so nothing on the network can raise this prompt.
    fn draw_approval_request(&mut self, ui: &mut egui::Ui, request: &crate::host::PendingPairing) {
        egui::Frame::new()
            .fill(Color32::from_rgb(28, 36, 48))
            .stroke(EguiStroke::new(1.0, Color32::from_rgb(226, 183, 80)))
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("\"{}\" wants to pair", request.device_name))
                        .size(16.0)
                        .strong(),
                );
                ui.label(
                    RichText::new(format!(
                        "{} at {}",
                        request.platform, request.peer_address
                    ))
                    .size(12.0)
                    .color(Color32::from_rgb(150, 162, 178)),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "Approve").clicked() {
                        self.host.decide_pending_pairing(true);
                        self.pairing_payload = None;
                        self.pairing_qr = None;
                        self.status = format!("Paired with {}", request.device_name);
                    }
                    if ui.button("Deny").clicked() {
                        self.host.decide_pending_pairing(false);
                        self.pairing_payload = None;
                        self.pairing_qr = None;
                        self.status = "Refused the pairing request".to_owned();
                    }
                });
            });
    }

    fn draw_device_list(&mut self, ui: &mut egui::Ui) {
        let devices = self.host.devices();
        ui.label(RichText::new("Paired devices").strong());
        if devices.is_empty() {
            ui.label(
                RichText::new("None yet. Uploads still arrive on the old mobile URL.")
                    .size(12.0)
                    .color(Color32::from_rgb(150, 162, 178)),
            );
            return;
        }

        for device in devices {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&device.device_name).strong());
                ui.label(
                    RichText::new(&device.platform)
                        .size(12.0)
                        .color(Color32::from_rgb(150, 162, 178)),
                );
                if ui.button("Revoke").clicked() {
                    self.status = match self.host.revoke(&device.device_id) {
                        Ok(()) => format!("Revoked {}", device.device_name),
                        Err(error) => format!("Could not revoke {}: {error}", device.device_name),
                    };
                }
            });
        }
    }

    fn draw_legacy_toggle(&mut self, ui: &mut egui::Ui) {
        let mut enabled = self.host.legacy_uploads_enabled();
        if ui
            .checkbox(&mut enabled, "Accept uploads on the old mobile URL")
            .changed()
        {
            self.status = match self.host.set_legacy_uploads_enabled(enabled) {
                Ok(()) if enabled => {
                    "The old mobile URL accepts uploads again — anyone on this network who knows it can write."
                        .to_owned()
                }
                Ok(()) => "The old mobile URL no longer accepts uploads".to_owned(),
                Err(error) => format!("Could not change that setting: {error}"),
            };
        }
        ui.label(
            RichText::new(
                "Pairing a device turns this off. The browser companion has no pairing step, \
                 so it needs this on.",
            )
            .size(12.0)
            .color(Color32::from_rgb(150, 162, 178)),
        );
    }

    fn start_pairing(&mut self, ctx: &egui::Context) {
        // Only the address the host believes it is reachable on. A machine with
        // several interfaces should offer all of them; the payload field is
        // already a list so that is a fill-in, not a wire change.
        let addresses = self
            .server()
            .map(|server| vec![server.base_url()])
            .unwrap_or_default();

        match self.host.arm_pairing(addresses) {
            Ok(payload) => {
                let text = serde_json::to_string(&payload).unwrap_or_default();
                self.pairing_qr = qr_texture(ctx, &text);
                self.pairing_payload = Some(payload);
                self.status = "Scan the code in the companion, then approve it here".to_owned();
            }
            Err(error) => self.status = format!("Could not start pairing: {error}"),
        }
    }

    fn draw_page_browser(&mut self, ui: &mut egui::Ui) {
        self.draw_register_head(ui);
        ui.add_space(10.0);

        if self.pages.is_empty() {
            self.draw_empty_page_browser(ui);
            return;
        }

        let pages = self.pages.clone();
        let pinned = self.pinned_page_id.clone();
        let numbers = sheet_numbers(&pages);
        let columns = ((ui.available_width() / (THUMBNAIL_WIDTH as f32 + 24.0)).floor() as usize)
            .clamp(1, 6);
        let mut stamping = None;
        let mut sending = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("pages")
                .spacing(Vec2::new(20.0, 20.0))
                .show(ui, |ui| {
                    for (index, page) in pages.iter().enumerate() {
                        let thumbnail = self.thumbnail_for(ui.ctx(), page);
                        let issued = pinned.as_deref() == Some(page.page_id.as_str());
                        let number = numbers.get(&page.page_id).copied().unwrap_or(index + 1);
                        let action = draw_sheet_card(ui, page, thumbnail, issued, number);
                        match action {
                            SheetAction::Stamp => {
                                stamping = Some(if issued {
                                    None
                                } else {
                                    Some(page.page_id.clone())
                                })
                            }
                            SheetAction::Send => sending = Some(page.page_id.clone()),
                            SheetAction::None => {}
                        }
                        if (index + 1) % columns == 0 {
                            ui.end_row();
                        }
                    }
                });
        });

        if let Some(target) = stamping {
            self.stamp_page(target.as_deref());
        }
        if let Some(page_id) = sending {
            self.send_page(&page_id);
        }
    }

    /// The register head: the one line saying which sheet the agent reads.
    fn draw_register_head(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("CLAUDE READS")
                    .size(10.0)
                    .strong()
                    .color(sheet(ui).ink_label),
            );
            match self.pinned_page_id.clone() {
                Some(page_id) => {
                    ui.label(
                        RichText::new(self.page_title(&page_id))
                            .size(13.0)
                            .strong()
                            .color(sheet(ui).ink),
                    );
                    if ui.button("Lift stamp").clicked() {
                        self.stamp_page(None);
                    }
                }
                None => {
                    ui.label(
                        RichText::new("whichever sheet was drawn on last")
                            .size(13.0)
                            .color(sheet(ui).ink_label),
                    );
                }
            }

            ui.separator();
            if ui.button("Refresh").clicked() {
                self.refresh_pages();
            }
            if ui.button("Reveal drawings folder").clicked() {
                self.reveal_drawings_dir();
            }
        });
    }

    fn draw_empty_page_browser(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("No pages yet")
                .size(16.0)
                .color(Color32::from_rgb(214, 220, 230)),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!(
                "Draw here or on the iPad and pages appear under {}.",
                pages_dir(&self.drawings_dir).display()
            ))
            .size(12.0)
            .color(Color32::from_rgb(150, 162, 178)),
        );
    }

}

enum SheetAction {
    None,
    Stamp,
    Send,
}

/// A sheet in the register: the drawing, then a ruled title block. Elevation is
/// declared once as a hairline edge — paper on paper does not glow.
fn draw_sheet_card(
    ui: &mut egui::Ui,
    page: &PageEntry,
    thumbnail: Option<TextureHandle>,
    issued: bool,
    number: usize,
) -> SheetAction {
    let card_width = THUMBNAIL_WIDTH as f32;
    let mut action = SheetAction::None;

    ui.allocate_ui(Vec2::new(card_width, THUMBNAIL_HEIGHT as f32 + 96.0), |ui| {
        egui::Frame::new()
            .fill(sheet(ui).paper)
            .stroke(EguiStroke::new(
                1.0,
                if issued { sheet(ui).stamp } else { sheet(ui).edge },
            ))
            .corner_radius(egui::CornerRadius::same(2))
            .inner_margin(egui::Margin::same(0))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    let image_size = Vec2::new(card_width, THUMBNAIL_HEIGHT as f32);
                    match thumbnail {
                        Some(handle) => {
                            ui.add(egui::Image::new(&handle).fit_to_exact_size(image_size));
                        }
                        None => {
                            ui.allocate_space(image_size);
                        }
                    }

                    // Top rule of the title block.
                    let (rule, painter) =
                        ui.allocate_painter(Vec2::new(card_width, 1.0), Sense::hover());
                    painter.rect_filled(rule.rect, 0.0, sheet(ui).rule);

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new("SHEET").size(9.0).strong().color(sheet(ui).ink_label));
                            ui.label(
                                RichText::new(format!("{number:02}"))
                                    .size(12.0)
                                    .monospace()
                                    .color(sheet(ui).ink),
                            );
                        });
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new("NAME").size(9.0).strong().color(sheet(ui).ink_label));
                            ui.label(
                                RichText::new(page.title.as_deref().unwrap_or(&page.page_id))
                                    .size(13.0)
                                    .strong()
                                    .color(sheet(ui).ink),
                            );
                        });
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(format!(
                                "{} · {} strokes",
                                relative_time(page.updated_at),
                                page.stroke_count
                            ))
                            .size(11.0)
                            .color(sheet(ui).ink_label),
                        );
                    });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        let stamp_label = if issued { "Lift stamp" } else { "Stamp" };
                        if ui.button(stamp_label).clicked() {
                            action = SheetAction::Stamp;
                        }
                        if ui.button("Send now").clicked() {
                            action = SheetAction::Send;
                        }
                        if issued {
                            ui.label(RichText::new("ISSUED").size(11.0).strong().color(sheet(ui).stamp));
                        }
                    });
                    ui.add_space(8.0);
                });
            });
    });

    action
}

fn relative_time(updated_at_ms: u128) -> String {
    let now = unix_millis();
    if updated_at_ms == 0 || updated_at_ms > now {
        return "just now".to_owned();
    }

    let seconds = (now - updated_at_ms) / 1000;
    match seconds {
        0..=59 => "just now".to_owned(),
        60..=3599 => format!("{} min ago", seconds / 60),
        3600..=86_399 => format!("{} hours ago", seconds / 3600),
        _ => format!("{} days ago", seconds / 86_400),
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

/// Registered once at startup rather than every frame. The old version ran from
/// `ui()` and hard-set `Visuals::dark()`, which is why the appearance was
/// unconditional — and why the paper register sat inside a navy shell looking
/// like a different application.
pub fn install_theme(ctx: &egui::Context) {
    ctx.set_visuals_of(egui::Theme::Light, chrome(&LIGHT, egui::Visuals::light()));
    ctx.set_visuals_of(egui::Theme::Dark, chrome(&DARK, egui::Visuals::dark()));
    ctx.set_theme(egui::ThemePreference::System);
}

/// egui's own chrome — panels, buttons, menus — dressed in the same palette the
/// sheets use, so nothing on screen belongs to a different set.
///
/// Two things here are load-bearing and were got wrong once already.
///
/// `noninteractive.fg_stroke` is the default colour of **every plain label**,
/// not a colour for secondary text. Setting it to the label grey is what made
/// the iPad settings screen unreadable before it was rebuilt at full ink weight
/// (DESIGN.md), and doing it here washed out the whole window. Body text is
/// ink; anything genuinely secondary asks for `ink_label` by name.
///
/// And a button needs an edge. Paper fill on a paper-coloured ground with no
/// stroke is not a quiet control, it is an invisible one — which is how the
/// connection chip managed to hide in plain sight.
fn chrome(sheet: &Sheet, mut visuals: egui::Visuals) -> egui::Visuals {
    visuals.window_fill = sheet.ground;
    visuals.panel_fill = sheet.ground;
    visuals.extreme_bg_color = sheet.paper;

    visuals.widgets.noninteractive.fg_stroke.color = sheet.ink;
    visuals.widgets.noninteractive.bg_stroke = EguiStroke::new(1.0, sheet.edge);

    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
    ] {
        widget.fg_stroke.color = sheet.ink;
        // DESIGN.md: control radius 8.
        widget.corner_radius = egui::CornerRadius::same(8);
    }
    visuals.widgets.inactive.bg_fill = sheet.paper;
    visuals.widgets.inactive.weak_bg_fill = sheet.paper;
    visuals.widgets.inactive.bg_stroke = EguiStroke::new(1.0, sheet.edge);
    visuals.widgets.hovered.bg_fill = sheet.paper;
    visuals.widgets.hovered.weak_bg_fill = sheet.paper;
    visuals.widgets.hovered.bg_stroke = EguiStroke::new(1.0, sheet.rule);
    visuals.widgets.active.bg_fill = sheet.edge;
    visuals.widgets.active.weak_bg_fill = sheet.edge;
    visuals.widgets.active.bg_stroke = EguiStroke::new(1.0, sheet.rule);

    visuals.selection.bg_fill = sheet.stamp;
    visuals.selection.stroke.color = sheet.paper;
    visuals.window_stroke = EguiStroke::new(1.0, sheet.edge);
    visuals.popup_shadow = egui::epaint::Shadow::NONE;
    visuals
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


/// Enough of the identity to tell two hosts apart on screen without printing a
/// 32-character string at someone.
fn short_host_id(host_id: &str) -> String {
    host_id
        .as_bytes()
        .chunks(4)
        .take(2)
        .map(|chunk| String::from_utf8_lossy(chunk).to_uppercase())
        .collect::<Vec<_>>()
        .join("-")
}

/// Renders the pairing payload as a QR code. Typing a 32-character secret on a
/// tablet is the kind of friction that stops people pairing at all.
fn qr_texture(ctx: &egui::Context, text: &str) -> Option<TextureHandle> {
    const MODULE_PIXELS: usize = 4;
    const QUIET_MODULES: usize = 4;

    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    let modules = code.to_colors();
    let width = code.width();
    let side = (width + QUIET_MODULES * 2) * MODULE_PIXELS;

    let mut pixels = vec![Color32::WHITE; side * side];
    for (index, module) in modules.iter().enumerate() {
        if *module != qrcode::Color::Dark {
            continue;
        }
        let module_x = (index % width + QUIET_MODULES) * MODULE_PIXELS;
        let module_y = (index / width + QUIET_MODULES) * MODULE_PIXELS;
        for y in module_y..module_y + MODULE_PIXELS {
            for x in module_x..module_x + MODULE_PIXELS {
                pixels[y * side + x] = Color32::BLACK;
            }
        }
    }

    let image = egui::ColorImage {
        size: [side, side],
        pixels,
        source_size: egui::Vec2::new(side as f32, side as f32),
    };
    Some(ctx.load_texture("pairing-qr", image, egui::TextureOptions::NEAREST))
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
