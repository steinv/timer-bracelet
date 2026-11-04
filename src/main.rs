// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod db;
mod device;
mod i18n;
mod protocol;

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime};
use eframe::egui::{self, Color32, RichText};
use egui_extras::DatePickerButton;

use db::{Db, Online};
use device::Event;
use i18n::Locale;

const GREEN: Color32 = Color32::from_rgb(0x2e, 0xa0, 0x43);
const BLUE: Color32 = Color32::from_rgb(0x2f, 0x6f, 0xd6);
const RED: Color32 = Color32::from_rgb(0xd0, 0x35, 0x35);

fn main() -> eframe::Result<()> {
    let loc = i18n::detect();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(loc.t.title)
            .with_inner_size([560.0, 520.0])
            .with_min_inner_size([460.0, 440.0]),
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

enum Dialog {
    Info(String),
    ConfirmDelete { from: NaiveDateTime, to: NaiveDateTime, n: i64 },
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
    yellow: u16,
    red: u16,
    write_mode: bool,
    running: bool,

    outgoing: device::Outgoing,
    events: Receiver<Event>,
    connected: bool,
    last_programmed: Option<NaiveDateTime>,

    online: Online,
    last_refresh: Option<Instant>,

    from: Range,
    to: Range,
    total: Option<i64>,

    dialog: Option<Dialog>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, loc: Locale) -> Self {
        let dir = data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let (db, mut dialog) = match Db::open(&dir.join("timer.db")) {
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
        let yellow = setting("yellow", 1);
        let red = setting("red", 1);
        let write_mode = db.as_ref().and_then(|d| d.get("mode")).as_deref() != Some("close");

        let outgoing: device::Outgoing = Arc::new(Mutex::new(None));
        let (tx, rx) = channel();
        let ctx = cc.egui_ctx.clone();
        device::spawn(outgoing.clone(), tx, move || ctx.request_repaint());

        let today = Local::now().date_naive();
        if db.is_none() && dialog.is_none() {
            dialog = Some(Dialog::Info(loc.t.db_error.into()));
        }

        Self {
            loc,
            db,
            green,
            yellow,
            red,
            write_mode,
            running: false,
            outgoing,
            events: rx,
            connected: false,
            last_programmed: None,
            online: Online::default(),
            last_refresh: None,
            from: Range { date: today, hour: 0, minute: 0 },
            to: Range { date: today, hour: 23, minute: 59 },
            total: None,
            dialog,
        }
    }

    fn command(&self) -> u8 {
        if self.write_mode {
            protocol::CMD_WRITE
        } else {
            protocol::CMD_CLOSE
        }
    }

    fn save_settings(&self) {
        if let Some(db) = &self.db {
            let _ = db.set("green", &self.green.to_string());
            let _ = db.set("yellow", &self.yellow.to_string());
            let _ = db.set("red", &self.red.to_string());
            let _ = db.set("mode", if self.write_mode { "write" } else { "close" });
        }
    }

    fn handle_events(&mut self) {
        while let Ok(ev) = self.events.try_recv() {
            match ev {
                Event::Connected(c) => {
                    self.connected = c;
                    if !c && self.running {
                        // Keep "running": sending resumes automatically on reconnect.
                    }
                }
                Event::Error(e) => {
                    self.dialog = Some(Dialog::Info(format!("{}: {e}", self.loc.t.device_error)));
                }
                Event::Reply { sent, received } => {
                    let (cmd, g, y, r) = protocol::decode(&sent);
                    if cmd != protocol::CMD_WRITE {
                        continue;
                    }
                    if received == sent {
                        let now = Local::now().naive_local();
                        if let Some(db) = &self.db {
                            if let Err(e) = db.insert_record(now, g, y, r) {
                                self.dialog =
                                    Some(Dialog::Info(format!("{}: {e}", self.loc.t.db_error)));
                            }
                        }
                        self.last_programmed = Some(now);
                        self.last_refresh = None;
                    } else {
                        self.dialog = Some(Dialog::Info(self.loc.t.checksum_error.into()));
                    }
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
        }
    }

    fn request_delete(&mut self) {
        let t = self.loc.t;
        let (from, to) = (self.from.at(), self.to.at());
        let today = Local::now().date_naive();
        if from.date() >= today || to.date() >= today {
            self.dialog = Some(Dialog::Info(t.no_delete_today.into()));
            return;
        }
        if from > to {
            self.dialog = Some(Dialog::Info(t.range_invalid.into()));
            return;
        }
        let n = self.db.as_ref().and_then(|d| d.count_between(from, to).ok()).unwrap_or(0);
        self.dialog = Some(Dialog::ConfirmDelete { from, to, n });
    }

    // ---------------------------------------------------------------- UI --

    fn ui_send(&mut self, ui: &mut egui::Ui) {
        let t = self.loc.t;
        ui.heading(t.send_command);
        let before = (self.green, self.yellow, self.red, self.write_mode);

        egui::Grid::new("minutes").num_columns(3).spacing([12.0, 6.0]).show(ui, |ui| {
            for (label, color, v) in [
                (t.green, GREEN, &mut self.green),
                (t.blue, BLUE, &mut self.yellow),
                (t.red, RED, &mut self.red),
            ] {
                ui.label(RichText::new(label).color(color).strong());
                ui.add(egui::DragValue::new(v).range(0..=999).speed(0.2));
                ui.label(t.minutes);
                ui.end_row();
            }
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.write_mode, true, t.write_timer);
            ui.radio_value(&mut self.write_mode, false, t.close_timer);
        });

        if before != (self.green, self.yellow, self.red, self.write_mode) {
            self.save_settings();
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let label = if self.running { t.stop } else { t.start };
            if ui.add(egui::Button::new(RichText::new(label).strong()).min_size([110.0, 30.0].into())).clicked() {
                if self.running {
                    self.running = false;
                } else if !self.connected {
                    self.dialog = Some(Dialog::Info(t.device_offline.into()));
                } else {
                    self.running = true;
                }
            }
            if self.running {
                ui.spinner();
                ui.label(t.sending);
            }
        });

        // Publish the current command to the device thread (re-sent every second).
        *self.outgoing.lock().unwrap() = self
            .running
            .then(|| protocol::build_payload(self.command(), self.green, self.yellow, self.red));
    }

    fn ui_online(&mut self, ui: &mut egui::Ui) {
        let t = self.loc.t;
        ui.heading(t.online);
        ui.horizontal(|ui| {
            for (label, color, n) in [
                (t.green, GREEN, self.online.green),
                (t.blue, BLUE, self.online.blue),
                (t.red, RED, self.online.red),
            ] {
                ui.label(RichText::new(format!("{label}:")).color(color).strong());
                ui.label(RichText::new(n.to_string()).size(20.0).strong());
                ui.add_space(16.0);
            }
        });
        if let Some(at) = self.last_programmed {
            ui.small(format!("{}: {}", t.last_programmed, at.format(self.loc.datetime_fmt)));
        }
    }

    fn ui_summary(&mut self, ui: &mut egui::Ui) {
        let t = self.loc.t;
        ui.heading(t.summary);
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
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button(t.check_total).clicked() {
                self.total = self
                    .db
                    .as_ref()
                    .and_then(|d| d.count_between(self.from.at(), self.to.at()).ok());
            }
            if ui.button(t.delete).clicked() {
                self.request_delete();
            }
            ui.label(format!("{}:", t.total));
            ui.label(RichText::new(self.total.map_or("–".into(), |n| n.to_string())).strong());
        });
    }

    fn ui_status(&mut self, ui: &mut egui::Ui) {
        let t = self.loc.t;
        ui.horizontal(|ui| {
            ui.label(format!("{} ({}):", t.device, t.communication.to_lowercase()));
            let (dot, text) = if self.connected { (GREEN, t.connected) } else { (RED, t.disconnected) };
            ui.label(RichText::new("●").color(dot));
            ui.label(text);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(Local::now().format(self.loc.datetime_fmt).to_string());
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
            self.total = self.db.as_ref().and_then(|d| d.count_between(from, to).ok());
            self.last_refresh = None;
        } else if close {
            self.dialog = None;
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_events();
        self.refresh_online();

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(4.0);
            self.ui_status(ui);
            ui.add_space(4.0);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    self.ui_send(ui);
                });
                ui.add_space(8.0);
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    self.ui_online(ui);
                });
                ui.add_space(8.0);
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    self.ui_summary(ui);
                });
            });
        });
        self.ui_dialog(ctx);

        // Clock and counters tick once a second even without input.
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}
