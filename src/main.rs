// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod db;
mod device;
mod i18n;
mod protocol;

use std::f32::consts::PI;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime};
use eframe::egui::{self, Color32, CursorIcon, Pos2, RichText, Sense, Stroke, Vec2};
use egui_extras::DatePickerButton;

use db::{Db, Online};
use device::Event;
use i18n::Locale;

const GREEN: Color32 = Color32::from_rgb(0x2e, 0xa0, 0x43);
const BLUE: Color32 = Color32::from_rgb(0x2f, 0x6f, 0xd6);
const RED: Color32 = Color32::from_rgb(0xd0, 0x35, 0x35);
const AMBER: Color32 = Color32::from_rgb(0xe0, 0x9a, 0x1a);

/// How long the "✔ Bracelet programmed" confirmation stays visible.
const FLASH: Duration = Duration::from_secs(4);

fn main() -> eframe::Result<()> {
    let loc = i18n::detect();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(loc.t.title)
            .with_inner_size([520.0, 760.0])
            .with_min_inner_size([440.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        loc.t.title,
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, loc)))),
    )
}

fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "TimerController")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Program,
    TurnOff,
}

enum Dialog {
    Info(String),
    ConfirmDelete { from: NaiveDateTime, to: NaiveDateTime, n: i64 },
}

/// How many history rows to list (the count above the list is always exact).
const HISTORY_ROWS: usize = 200;

#[derive(Default)]
struct History {
    key: Option<(NaiveDateTime, NaiveDateTime)>,
    count: i64,
    rows: Vec<db::HistoryRow>,
}

struct Range {
    date: NaiveDate,
    hour: u32,
    minute: u32,
}

impl Range {
    fn at(&self) -> NaiveDateTime {
        self.date
            .and_time(NaiveTime::from_hms_opt(self.hour, self.minute, 0).unwrap_or_default())
    }
}

struct App {
    loc: Locale,
    db: Option<Db>,

    green: u16,
    blue: u16,
    red: u16,
    mode: Mode,
    running: bool,

    outgoing: device::Outgoing,
    events: Receiver<Event>,
    connected: bool,

    /// Time of the last confirmed bracelet + what happened to it.
    flash: Option<(Instant, Mode)>,
    session_count: u32,
    last_programmed: Option<NaiveDateTime>,

    online: Online,
    last_refresh: Option<Instant>,

    from: Range,
    to: Range,
    history: History,

    dialog: Option<Dialog>,
    /// Whether the History section is expanded (remembered between runs).
    history_open: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, loc: Locale) -> Self {
        install_fallback_font(&cc.egui_ctx);
        let dir = data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let (db, dialog) = match Db::open(&dir.join("timer.db")) {
            Ok(db) => (Some(db), None),
            Err(e) => (None, Some(Dialog::Info(format!("{}: {e}", loc.t.db_error)))),
        };

        let setting = |k: &str, def: u16| {
            db.as_ref()
                .and_then(|d| d.get(k))
                .and_then(|v| v.parse().ok())
                .unwrap_or(def)
        };
        let green = setting("green", 1);
        let blue = setting("yellow", 1);
        let red = setting("red", 1);
        let history_open = db.as_ref().and_then(|d| d.get("history_open")).as_deref() != Some("0");
        let mode = if db.as_ref().and_then(|d| d.get("mode")).as_deref() == Some("close") {
            Mode::TurnOff
        } else {
            Mode::Program
        };

        let outgoing: device::Outgoing = Arc::new(Mutex::new(None));
        let (tx, rx) = channel();
        let ctx = cc.egui_ctx.clone();
        device::spawn(outgoing.clone(), tx, move || ctx.request_repaint());

        let today = Local::now().date_naive();
        Self {
            loc,
            db,
            green,
            blue,
            red,
            mode,
            running: false,
            outgoing,
            events: rx,
            connected: false,
            flash: None,
            session_count: 0,
            last_programmed: None,
            online: Online::default(),
            last_refresh: None,
            from: Range { date: today, hour: 0, minute: 0 },
            to: Range { date: today, hour: 23, minute: 59 },
            history: History::default(),
            dialog,
            history_open,
        }
    }

    fn payload(&self) -> protocol::Payload {
        match self.mode {
            Mode::Program => protocol::build_payload(protocol::CMD_WRITE, self.green, self.blue, self.red),
            Mode::TurnOff => protocol::turn_off_payload(),
        }
    }

    fn save_settings(&self) {
        if let Some(db) = &self.db {
            let _ = db.set("green", &self.green.to_string());
            let _ = db.set("yellow", &self.blue.to_string());
            let _ = db.set("red", &self.red.to_string());
            let _ = db.set("mode", if self.mode == Mode::Program { "write" } else { "close" });
        }
    }

    fn handle_events(&mut self) {
        while let Ok(ev) = self.events.try_recv() {
            match ev {
                Event::Connected(c) => self.connected = c,
                Event::Error(e) => {
                    self.dialog = Some(Dialog::Info(format!("{}: {e}", self.loc.t.device_error)));
                }
                Event::Reply { matched: Some(sent), .. } => {
                    let (_, g, b, r) = protocol::decode(&sent);
                    let now = Local::now().naive_local();
                    let result = if protocol::is_turn_off(&sent) {
                        self.flash = Some((Instant::now(), Mode::TurnOff));
                        self.db.as_ref().map(|db| db.end_one_active(now).map(|_| ()))
                    } else {
                        self.last_programmed = Some(now);
                        self.flash = Some((Instant::now(), Mode::Program));
                        self.db.as_ref().map(|db| db.insert_record(now, g, b, r))
                    };
                    if let Some(Err(e)) = result {
                        self.dialog = Some(Dialog::Info(format!("{}: {e}", self.loc.t.db_error)));
                    }
                    self.last_refresh = None; // update counters right away
                    self.session_count += 1;
                }
                Event::Reply { matched: None, .. } => {
                    self.dialog = Some(Dialog::Info(self.loc.t.checksum_error.into()));
                }
            }
        }
    }

    fn refresh_online(&mut self) {
        if self.last_refresh.map_or(true, |t| t.elapsed() >= Duration::from_secs(1)) {
            if let Some(db) = &self.db {
                if let Ok(o) = db.online(Local::now().naive_local()) {
                    self.online = o;
                }
            }
            self.last_refresh = Some(Instant::now());
            self.refresh_history(true);
        }
    }

    fn toggle_power(&mut self) {
        if self.running {
            self.running = false;
        } else if !self.connected {
            self.dialog = Some(Dialog::Info(self.loc.t.device_offline.into()));
        } else {
            self.running = true;
            self.session_count = 0;
            self.flash = None;
        }
    }

    // ---------------------------------------------------------------- UI --

    fn ui_mode(&mut self, ui: &mut egui::Ui) {
        let t = self.loc.t;
        let before = self.mode;
        ui.add_enabled_ui(!self.running, |ui| {
            // [Program bracelets]  (switch)  [Turn bracelets off]
            ui.columns(3, |cols| {
                let label = |ui: &mut egui::Ui, text: &str, mode: Mode, color: Color32, current: Mode| {
                    let active = current == mode;
                    let mut rt = RichText::new(text).size(15.0);
                    rt = if active { rt.strong().color(color) } else { rt.weak() };
                    ui.add(egui::Label::new(rt).sense(Sense::click()).selectable(false))
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                };
                // Fixed row height: a centred layout inside the scroll area
                // would otherwise grow to fill all available height.
                const ROW: f32 = 36.0;
                let row = |ui: &egui::Ui| Vec2::new(ui.available_width(), ROW);

                let size = row(&cols[0]);
                cols[0].allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if label(ui, t.mode_program, Mode::Program, GREEN, self.mode) {
                        self.mode = Mode::Program;
                    }
                });
                let size = row(&cols[1]);
                cols[1].allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add_space(1.0);
                    if toggle_switch(ui, self.mode == Mode::TurnOff).clicked() {
                        self.mode = if self.mode == Mode::Program { Mode::TurnOff } else { Mode::Program };
                    }
                });
                let size = row(&cols[2]);
                cols[2].allocate_ui_with_layout(size, egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if label(ui, t.mode_off, Mode::TurnOff, RED, self.mode) {
                        self.mode = Mode::TurnOff;
                    }
                });
            });
        });

        // The program description sits under "Duration per colour" instead.
        if self.mode == Mode::TurnOff {
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(t.mode_off_desc).weak());
            });
        }

        if before != self.mode {
            self.save_settings();
        }
    }

    fn ui_durations(&mut self, ui: &mut egui::Ui) {
        let t = self.loc.t;
        let before = (self.green, self.blue, self.red);
        ui.label(RichText::new(t.durations).strong());
        let desc = t
            .mode_program_desc
            .replace("{g}", &self.green.to_string())
            .replace("{b}", &self.blue.to_string())
            .replace("{r}", &self.red.to_string());
        ui.label(RichText::new(desc).weak());
        ui.add_space(4.0);
        ui.add_enabled_ui(!self.running, |ui| {
            egui::Grid::new("durations").num_columns(3).spacing([10.0, 8.0]).show(ui, |ui| {
                for (label, color, v) in [
                    (t.green, GREEN, &mut self.green),
                    (t.blue, BLUE, &mut self.blue),
                    (t.red, RED, &mut self.red),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("●").color(color).size(18.0));
                        ui.label(RichText::new(label).size(15.0));
                    });
                    ui.horizontal(|ui| {
                        let step = Vec2::new(30.0, 26.0);
                        if ui.add_sized(step, egui::Button::new(RichText::new("−").size(16.0))).clicked() {
                            *v = v.saturating_sub(1);
                        }
                        minutes_field(ui, label, v);
                        if ui.add_sized(step, egui::Button::new(RichText::new("+").size(16.0))).clicked() {
                            *v = (*v + 1).min(999);
                        }
                    });
                    ui.label(t.minutes);
                    ui.end_row();
                }
            });
        });
        if before != (self.green, self.blue, self.red) {
            self.save_settings();
        }
    }

    fn ui_magnet_hint(&self, ui: &mut egui::Ui) {
        let t = self.loc.t;
        let dark = ui.visuals().dark_mode;
        let fill = if dark {
            Color32::from_rgb(0x3a, 0x2f, 0x12)
        } else {
            Color32::from_rgb(0xfd, 0xf3, 0xd8)
        };
        egui::Frame::group(ui.style())
            .fill(fill)
            .stroke(Stroke::new(1.0_f32, AMBER))
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                    paint_magnet(ui, rect);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(t.magnet_title).strong());
                        ui.label(t.magnet_hint);
                    });
                });
            });
    }

    fn ui_power(&mut self, ui: &mut egui::Ui) {
        let t = self.loc.t;
        ui.vertical_centered(|ui| {
            let size = 110.0;
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size + 24.0), Sense::click());
            let resp = resp
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(if self.running { t.power_tooltip_stop } else { t.power_tooltip_start });
            if resp.clicked() {
                self.toggle_power();
            }
            paint_power_button(ui, rect.center(), size / 2.0, self.running, self.mode, resp.hovered());

            let status = match (self.running, self.mode) {
                (false, _) => t.power_off,
                (true, Mode::Program) => t.power_on_program,
                (true, Mode::TurnOff) => t.power_on_off,
            };
            ui.label(RichText::new(status).size(16.0).strong());

            if self.running {
                ui.label(RichText::new(t.locked_hint).weak().small());
                ui.add_space(4.0);
                if let Some((at, what)) = self.flash.filter(|(at, _)| at.elapsed() < FLASH) {
                    let _ = at;
                    let (msg, color) = match what {
                        Mode::Program => (t.ok_programmed, GREEN),
                        Mode::TurnOff => (t.ok_turned_off, RED),
                    };
                    ui.label(RichText::new(msg).size(18.0).strong().color(color));
                }
                ui.label(t.session_count.replace("{n}", &self.session_count.to_string()));
            }
        });
    }

    fn ui_active(&mut self, ui: &mut egui::Ui) {
        let t = self.loc.t;
        let dark = ui.visuals().dark_mode;
        ui.columns(3, |cols| {
            for (i, (label, color, n)) in [
                (t.green, GREEN, self.online.green),
                (t.blue, BLUE, self.online.blue),
                (t.red, RED, self.online.red),
            ]
            .into_iter()
            .enumerate()
            {
                // Tinted tile; dimmed when nothing is in this phase.
                let active = n > 0;
                let tint = match (dark, active) {
                    (true, true) => 0.28,
                    (true, false) => 0.10,
                    (false, true) => 0.20,
                    (false, false) => 0.07,
                };
                let fill = color.gamma_multiply(tint);
                let number_color = if active { color } else { color.gamma_multiply(0.55) };
                egui::Frame::new()
                    .fill(fill)
                    .corner_radius(12.0)
                    .inner_margin(egui::Margin::symmetric(8, 12))
                    .show(&mut cols[i], |ui| {
                        ui.set_width(ui.available_width());
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(n.to_string()).size(36.0).strong().color(number_color));
                            ui.horizontal(|ui| {
                                // centre "● Label" under the number
                                let text_w = ui.fonts(|f| {
                                    f.layout_no_wrap(format!("● {label}"), egui::FontId::proportional(14.0), Color32::WHITE)
                                        .size()
                                        .x
                                });
                                ui.add_space(((ui.available_width() - text_w) / 2.0).max(0.0));
                                ui.spacing_mut().item_spacing.x = 4.0;
                                ui.label(RichText::new("●").size(14.0).color(color));
                                ui.label(RichText::new(label).size(14.0));
                            });
                        });
                    });
            }
        });
        if let Some(at) = self.last_programmed {
            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(format!("{}: {}", t.last_programmed, at.format(self.loc.datetime_fmt)))
                        .weak()
                        .small(),
                );
            });
        }
    }

    fn set_range(&mut self, from: NaiveDate, to: NaiveDate) {
        self.from = Range { date: from, hour: 0, minute: 0 };
        self.to = Range { date: to, hour: 23, minute: 59 };
    }

    /// Re-query the history when the range changed (or once a second, so
    /// newly programmed bracelets show up immediately).
    fn refresh_history(&mut self, force: bool) {
        let key = (self.from.at(), self.to.at());
        if !force && self.history.key == Some(key) {
            return;
        }
        if let Some(db) = &self.db {
            self.history.count = db.count_between(key.0, key.1).unwrap_or(0);
            self.history.rows = db.list_between(key.0, key.1, HISTORY_ROWS).unwrap_or_default();
        }
        self.history.key = Some(key);
    }

    fn ui_history(&mut self, ui: &mut egui::Ui) {
        let t = self.loc.t;
        let resp = egui::CollapsingHeader::new(RichText::new(t.history).strong())
            .open(Some(self.history_open))
            .show(ui, |ui| {
                // Quick period presets.
                let today = Local::now().date_naive();
                let presets = [
                    (t.preset_today, today, today),
                    (t.preset_yesterday, today - chrono::Days::new(1), today - chrono::Days::new(1)),
                    (t.preset_week, today - chrono::Days::new(6), today),
                    (
                        t.preset_before_today,
                        NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
                        today - chrono::Days::new(1),
                    ),
                ];
                ui.horizontal_wrapped(|ui| {
                    for (label, from, to) in presets {
                        let active = self.from.date == from
                            && self.to.date == to
                            && (self.from.hour, self.from.minute, self.to.hour, self.to.minute) == (0, 0, 23, 59);
                        if ui.selectable_label(active, label).clicked() {
                            self.set_range(from, to);
                        }
                    }
                });
                ui.add_space(6.0);

                // Custom range.
                let fmt = self.loc.date_fmt;
                egui::Grid::new("range").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    for (label, id, r) in [(t.from, "from", &mut self.from), (t.to, "to", &mut self.to)] {
                        ui.label(label);
                        ui.horizontal(|ui| {
                            ui.add(DatePickerButton::new(&mut r.date).id_salt(id).format(fmt).calendar_week(false));
                            ui.add(egui::DragValue::new(&mut r.hour).range(0..=23).custom_formatter(|v, _| format!("{v:02}")));
                            ui.label(":");
                            ui.add(egui::DragValue::new(&mut r.minute).range(0..=59).custom_formatter(|v, _| format!("{v:02}")));
                        });
                        ui.end_row();
                    }
                });
                self.refresh_history(false);
                ui.add_space(8.0);

                // Live result.
                let (from, to) = (self.from.at(), self.to.at());
                if from > to {
                    ui.label(RichText::new(t.range_invalid).color(RED));
                    return;
                }
                let n = self.history.count;
                ui.label(RichText::new(t.history_count.replace("{n}", &n.to_string())).size(15.0).strong());

                if n == 0 {
                    ui.label(RichText::new(t.history_empty).weak());
                } else {
                    ui.add_space(4.0);
                    egui::Frame::group(ui.style()).inner_margin(6.0).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                            egui::Grid::new("history_rows").num_columns(2).striped(true).spacing([24.0, 4.0]).show(ui, |ui| {
                                ui.label(RichText::new(t.col_time).weak());
                                ui.label(RichText::new(t.col_minutes).weak());
                                ui.end_row();
                                for row in &self.history.rows {
                                    ui.label(row.start.format(self.loc.datetime_fmt).to_string());
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;
                                        ui.label(RichText::new(row.green.to_string()).color(GREEN).strong());
                                        ui.label("/");
                                        ui.label(RichText::new(row.blue.to_string()).color(BLUE).strong());
                                        ui.label("/");
                                        ui.label(RichText::new(row.red.to_string()).color(RED).strong());
                                        if let Some(end) = row.ended {
                                            ui.add_space(8.0);
                                            ui.label(RichText::new(t.ended_at.replace("{t}", &end.format("%H:%M").to_string())).weak().italics());
                                        }
                                    });
                                    ui.end_row();
                                }
                            });
                            let more = n - self.history.rows.len() as i64;
                            if more > 0 {
                                ui.label(RichText::new(t.history_more.replace("{n}", &more.to_string())).weak());
                            }
                        });
                    });
                }

                // Delete, with the reason shown inline when it isn't allowed.
                ui.add_space(6.0);
                let touches_today = from.date() >= today || to.date() >= today;
                let can_delete = n > 0 && !touches_today;
                ui.horizontal_wrapped(|ui| {
                    let label = if n > 0 { t.delete_n.replace("{n}", &n.to_string()) } else { t.delete.to_string() };
                    let btn = egui::Button::new(RichText::new(label).color(if can_delete { RED } else { ui.visuals().weak_text_color() }));
                    if ui.add_enabled(can_delete, btn).clicked() {
                        self.dialog = Some(Dialog::ConfirmDelete { from, to, n });
                    }
                    if n > 0 && touches_today {
                        ui.label(RichText::new(t.delete_blocked_today).weak().small());
                    }
                });
            });
        if resp.header_response.clicked() {
            self.history_open = !self.history_open;
            if let Some(db) = &self.db {
                let _ = db.set("history_open", if self.history_open { "1" } else { "0" });
            }
        }
    }

    fn ui_status(&mut self, ui: &mut egui::Ui) {
        let t = self.loc.t;
        ui.horizontal(|ui| {
            let (dot, text) = if self.connected { (GREEN, t.connected) } else { (RED, t.disconnected) };
            ui.label(RichText::new("●").color(dot));
            ui.label(format!("{}: {text}", t.dongle));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(Local::now().format(self.loc.datetime_fmt).to_string()).weak());
            });
        });
    }

    fn ui_dialog(&mut self, ctx: &egui::Context) {
        let t = self.loc.t;
        let Some(dialog) = &self.dialog else { return };
        let mut close = false;
        let mut delete: Option<(NaiveDateTime, NaiveDateTime)> = None;

        egui::Modal::new(egui::Id::new("dialog")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.heading(t.notice);
            ui.add_space(6.0);
            match dialog {
                Dialog::Info(msg) => {
                    ui.label(msg);
                    ui.add_space(8.0);
                    if ui.button(t.ok).clicked() {
                        close = true;
                    }
                }
                Dialog::ConfirmDelete { from, to, n } => {
                    ui.label(t.confirm_delete.replace("{n}", &n.to_string()));
                    ui.small(format!(
                        "{} – {}",
                        from.format(self.loc.datetime_fmt),
                        to.format(self.loc.datetime_fmt)
                    ));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new(t.delete).color(RED)).clicked() {
                            delete = Some((*from, *to));
                        }
                        if ui.button(t.cancel).clicked() {
                            close = true;
                        }
                    });
                }
            }
        });

        if let Some((from, to)) = delete {
            self.dialog = Some(Dialog::Info(match self.db.as_ref().map(|d| d.delete_between(from, to)) {
                Some(Ok(n)) => t.deleted.replace("{n}", &n.to_string()),
                Some(Err(e)) => format!("{}: {e}", t.db_error),
                None => t.db_error.into(),
            }));
            self.refresh_history(true);
            self.last_refresh = None;
        } else if close {
            self.dialog = None;
        }
    }
}

/// Round power button with the standard power symbol. Pulses while on.
/// Text field for 0–999 minutes that only accepts digits. Anything else typed
/// is dropped immediately; an empty field falls back to the last valid value
/// when it loses focus. Up/Down arrow keys step the value.
fn minutes_field(ui: &mut egui::Ui, id: &str, value: &mut u16) {
    let id = ui.id().with(("minutes", id));
    let mut text: String = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| value.to_string());

    let resp = ui.add_sized(
        Vec2::new(56.0, 26.0),
        egui::TextEdit::singleline(&mut text)
            .id(id)
            .char_limit(3)
            .horizontal_align(egui::Align::Center),
    );

    if resp.changed() {
        text = digits_only(&text);
        if let Ok(v) = text.parse::<u16>() {
            *value = v.min(999);
        }
    }
    if resp.has_focus() {
        let (up, down) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown)));
        if up || down {
            *value = if up { (*value + 1).min(999) } else { value.saturating_sub(1) };
            text = value.to_string();
        }
    } else {
        // Not editing: always show the real value (also after −/+ clicks).
        text = value.to_string();
    }
    ui.data_mut(|d| d.insert_temp(id, text));
}

/// iOS/Android-style switch. Knob left + green = program, knob right + red =
/// turn off. The knob slides between the two positions.
fn toggle_switch(ui: &mut egui::Ui, right: bool) -> egui::Response {
    let size = Vec2::new(64.0, 34.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool(resp.id, right); // 0.0 = left, 1.0 = right
        let track = lerp_color(GREEN, RED, t);
        let track = if ui.is_enabled() { track } else { track.gamma_multiply(0.45) };
        let radius = rect.height() / 2.0;
        let p = ui.painter();
        p.rect_filled(rect, radius, track);
        let knob_r = radius - 3.0;
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
        let knob = Pos2::new(x, rect.center().y);
        p.circle_filled(knob + Vec2::new(0.0, 1.0), knob_r, Color32::from_black_alpha(50)); // shadow
        p.circle_filled(knob, knob_r, Color32::WHITE);
    }
    resp
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// Keep only ASCII digits, at most 3 of them (minutes are 0–999).
fn digits_only(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).take(3).collect()
}

#[cfg(test)]
mod tests {
    use super::digits_only;

    #[test]
    fn minutes_input_accepts_only_digits() {
        assert_eq!(digits_only("ab5x9q"), "59");
        assert_eq!(digits_only("12"), "12");
        assert_eq!(digits_only("-3"), "3");
        assert_eq!(digits_only("1.5"), "15");
        assert_eq!(digits_only("12345"), "123");
        assert_eq!(digits_only("٣"), ""); // non-ASCII digits are rejected too
        assert_eq!(digits_only(""), "");
    }
}

/// egui's built-in fonts miss symbols such as ● and ✔ (they render as empty
/// boxes). DejaVu Sans covers them, so add it as a fallback after the
/// default fonts. It is embedded, so it works the same on every OS.
fn install_fallback_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "dejavu".into(),
        Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/DejaVuSans.ttf"))),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push("dejavu".into());
    }
    ctx.set_fonts(fonts);
}

/// Small horseshoe magnet icon (drawn, so it doesn't depend on font glyphs).
fn paint_magnet(ui: &egui::Ui, rect: egui::Rect) {
    let p = ui.painter();
    let w = rect.width() * 0.2; // stroke width
    let r = rect.width() * 0.28; // radius of the bend
    let c = Pos2::new(rect.center().x, rect.center().y + rect.height() * 0.1);
    let top = rect.top() + w * 0.6;
    let pole = top + w * 1.3; // where the grey tips end
    let body = Stroke::new(w, RED);
    let tip = Stroke::new(w, Color32::from_gray(150));

    // U-shaped body: two legs joined by a half circle at the bottom.
    let mut points = vec![Pos2::new(c.x - r, pole)];
    points.extend((0..=24).map(|i| {
        let a = PI - PI * i as f32 / 24.0; // left (PI) -> bottom -> right (0)
        c + Vec2::new(a.cos(), a.sin()) * r
    }));
    points.push(Pos2::new(c.x + r, pole));
    p.add(egui::Shape::line(points, body));

    // Grey pole tips.
    for x in [c.x - r, c.x + r] {
        p.line_segment([Pos2::new(x, top), Pos2::new(x, pole)], tip);
    }
}

fn paint_power_button(ui: &egui::Ui, c: Pos2, radius: f32, on: bool, mode: Mode, hovered: bool) {
    let p = ui.painter();
    let v = ui.visuals();
    let accent = if mode == Mode::Program { GREEN } else { RED };

    if on {
        // Expanding, fading ring.
        let phase = (ui.input(|i| i.time) % 1.6 / 1.6) as f32;
        let ring = radius + 4.0 + phase * 10.0;
        let alpha = ((1.0 - phase) * 160.0) as u8;
        p.circle_stroke(c, ring, Stroke::new(3.0_f32, Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), alpha)));
        ui.ctx().request_repaint();
    }

    let fill = if on {
        if hovered { accent.gamma_multiply(0.85) } else { accent }
    } else if hovered {
        v.widgets.hovered.bg_fill
    } else {
        v.widgets.inactive.bg_fill
    };
    p.circle_filled(c, radius, fill);
    p.circle_stroke(c, radius, Stroke::new(2.0_f32, if on { accent } else { v.widgets.inactive.fg_stroke.color }));

    // Power symbol: an open arc with a vertical bar through the gap.
    let icon = if on { Color32::WHITE } else { v.text_color() };
    let r = radius * 0.42;
    let gap = 40.0_f32.to_radians();
    let start = -PI / 2.0 + gap;
    let end = 3.0 * PI / 2.0 - gap;
    let points: Vec<Pos2> = (0..=48)
        .map(|i| {
            let a = start + (end - start) * i as f32 / 48.0;
            c + Vec2::new(a.cos(), a.sin()) * r
        })
        .collect();
    let stroke = Stroke::new(radius * 0.1, icon);
    p.add(egui::Shape::line(points, stroke));
    p.line_segment([c - Vec2::new(0.0, r * 1.2), c - Vec2::new(0.0, r * 0.15)], stroke);
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_events();
        self.refresh_online();

        // Publish the current command to the device thread (re-sent every second).
        *self.outgoing.lock().unwrap() = self.running.then(|| self.payload());

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(4.0);
            self.ui_status(ui);
            ui.add_space(4.0);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(4.0);
                ui.add_space(4.0);
                self.ui_active(ui);
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(10.0);
                self.ui_mode(ui);
                ui.add_space(12.0);
                if self.mode == Mode::Program {
                    self.ui_durations(ui);
                    ui.add_space(12.0);
                }
                ui.add_space(4.0);
                self.ui_power(ui);
                if self.running {
                    // Only relevant while the dongle is sending.
                    ui.add_space(12.0);
                    self.ui_magnet_hint(ui);
                }
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);
                self.ui_history(ui);
            });
        });
        self.ui_dialog(ctx);

        // Clock, counters and the confirmation message update without input.
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}
