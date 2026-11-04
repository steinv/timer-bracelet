//! English + Dutch UI text. The language and date format follow the system
//! locale; set TIMER_LANG=en or TIMER_LANG=nl to override.
//!
//! Placeholders: `{n}` = a number, `{g}` `{b}` `{r}` = minutes per colour.

pub struct Texts {
    pub title: &'static str,

    // mode toggle
    pub mode_program: &'static str,
    pub mode_off: &'static str,
    pub mode_program_desc: &'static str,
    pub mode_off_desc: &'static str,

    // durations
    pub durations: &'static str,
    pub green: &'static str,
    pub blue: &'static str,
    pub red: &'static str,
    pub minutes: &'static str,

    // magnet hint
    pub magnet_title: &'static str,
    pub magnet_hint: &'static str,

    // power button
    pub power_off: &'static str,
    pub power_on_program: &'static str,
    pub power_on_off: &'static str,
    pub power_tooltip_start: &'static str,
    pub power_tooltip_stop: &'static str,
    pub locked_hint: &'static str,
    pub ok_programmed: &'static str,
    pub ok_turned_off: &'static str,
    pub session_count: &'static str,

    // active bracelets
    pub last_programmed: &'static str,

    // history
    pub history: &'static str,
    pub from: &'static str,
    pub to: &'static str,
    pub preset_today: &'static str,
    pub preset_yesterday: &'static str,
    pub preset_week: &'static str,
    pub preset_before_today: &'static str,
    pub history_count: &'static str,
    pub history_empty: &'static str,
    pub history_more: &'static str,
    pub col_time: &'static str,
    pub col_status: &'static str,
    pub ended_at: &'static str,
    pub delete: &'static str,
    pub delete_n: &'static str,
    pub delete_keeps_running: &'static str,

    // status + dialogs
    pub dongle: &'static str,
    pub connected: &'static str,
    pub disconnected: &'static str,
    pub device_offline: &'static str,
    pub checksum_error: &'static str,
    pub range_invalid: &'static str,
    pub confirm_delete: &'static str,
    pub deleted: &'static str,
    pub notice: &'static str,
    pub ok: &'static str,
    pub cancel: &'static str,
    pub db_error: &'static str,
    pub device_error: &'static str,
}

pub const EN: Texts = Texts {
    title: "Timer Controller",

    mode_program: "Program bracelets",
    mode_off: "Turn bracelets off",
    mode_program_desc: "Each bracelet gets {g} min green, then {b} min blue, then {r} min red.",
    mode_off_desc: "Each bracelet's timer is stopped and its light goes off.",

    durations: "Duration per colour",
    green: "Green",
    blue: "Blue",
    red: "Red",
    minutes: "min",

    magnet_title: "Activate each bracelet with the magnet",
    magnet_hint: "Hold the magnet against the bracelet until its light flickers, then hold the bracelet next to the USB dongle.",

    power_off: "Start",
    power_on_program: "On: programming bracelets…",
    power_on_off: "On: turning bracelets off…",
    power_tooltip_start: "Start sending to bracelets",
    power_tooltip_stop: "Stop sending",
    locked_hint: "Switch off to change the settings.",
    ok_programmed: "✔ Bracelet programmed",
    ok_turned_off: "✔ Bracelet turned off",
    session_count: "This session: {n}",

    last_programmed: "Last programmed",

    history: "History",
    from: "From",
    to: "To",
    preset_today: "Today",
    preset_yesterday: "Yesterday",
    preset_week: "Last 7 days",
    preset_before_today: "Everything before today",
    history_count: "{n} bracelets programmed in this period",
    history_empty: "No bracelets were programmed in this period.",
    history_more: "… and {n} more",
    col_time: "Time",
    col_status: "Status",
    ended_at: "turned off at {t}",
    delete: "Delete",
    delete_n: "Delete {n} records",
    delete_keeps_running: "{n} bracelets that are still running are kept.",

    dongle: "USB dongle",
    connected: "Connected",
    disconnected: "Not connected",
    device_offline: "The USB dongle is not connected. Plug it in and try again.",
    checksum_error: "The dongle sent an unexpected reply. Please try the bracelet again.",
    range_invalid: "The start must be before the end.",
    confirm_delete: "Delete {n} records?",
    deleted: "Deleted {n} records.",
    notice: "Notice",
    ok: "OK",
    cancel: "Cancel",
    db_error: "Database error",
    device_error: "Device error",
};

pub const NL: Texts = Texts {
    title: "Timer Controller",

    mode_program: "Armbanden programmeren",
    mode_off: "Armbanden uitzetten",
    mode_program_desc: "Elke armband brandt {g} min groen, daarna {b} min blauw en dan {r} min rood.",
    mode_off_desc: "De timer van elke armband wordt gestopt en het lampje gaat uit.",

    durations: "Duur per kleur",
    green: "Groen",
    blue: "Blauw",
    red: "Rood",
    minutes: "min",

    magnet_title: "Activeer elke armband met de magneet",
    magnet_hint: "Houd de magneet tegen de armband tot het lampje flikkert en houd de armband dan bij de USB-dongle.",

    power_off: "Start",
    power_on_program: "Aan: armbanden worden geprogrammeerd…",
    power_on_off: "Aan: armbanden worden uitgezet…",
    power_tooltip_start: "Begin met verzenden naar armbanden",
    power_tooltip_stop: "Stop met verzenden",
    locked_hint: "Zet uit om de instellingen te wijzigen.",
    ok_programmed: "✔ Armband geprogrammeerd",
    ok_turned_off: "✔ Armband uitgezet",
    session_count: "Deze sessie: {n}",

    last_programmed: "Laatst geprogrammeerd",

    history: "Geschiedenis",
    from: "Van",
    to: "Tot",
    preset_today: "Vandaag",
    preset_yesterday: "Gisteren",
    preset_week: "Laatste 7 dagen",
    preset_before_today: "Alles vóór vandaag",
    history_count: "{n} armbanden geprogrammeerd in deze periode",
    history_empty: "In deze periode zijn geen armbanden geprogrammeerd.",
    history_more: "… en nog {n}",
    col_time: "Tijdstip",
    col_status: "Status",
    ended_at: "uitgezet om {t}",
    delete: "Verwijderen",
    delete_n: "{n} records verwijderen",
    delete_keeps_running: "{n} armbanden die nog actief zijn blijven staan.",

    dongle: "USB-dongle",
    connected: "Verbonden",
    disconnected: "Niet verbonden",
    device_offline: "De USB-dongle is niet aangesloten. Sluit hem aan en probeer opnieuw.",
    checksum_error: "De dongle gaf een onverwacht antwoord. Probeer de armband opnieuw.",
    range_invalid: "Het begin moet vóór het einde liggen.",
    confirm_delete: "{n} records verwijderen?",
    deleted: "{n} records verwijderd.",
    notice: "Melding",
    ok: "OK",
    cancel: "Annuleren",
    db_error: "Databasefout",
    device_error: "Apparaatfout",
};

pub struct Locale {
    pub t: &'static Texts,
    /// chrono format string for dates, e.g. "%d-%m-%Y".
    pub date_fmt: &'static str,
    /// chrono format string for date + time.
    pub datetime_fmt: &'static str,
}

pub fn detect() -> Locale {
    let tag = std::env::var("TIMER_LANG")
        .ok()
        .or_else(sys_locale::get_locale)
        .unwrap_or_else(|| "en".into())
        .replace('_', "-")
        .to_ascii_lowercase();
    let lang = tag.split(['-', '.']).next().unwrap_or("en");
    let region = tag.split('-').nth(1).unwrap_or("");

    let t = if lang == "nl" { &NL } else { &EN };
    let (date_fmt, datetime_fmt) = match (lang, region) {
        ("nl", _) => ("%d-%m-%Y", "%d-%m-%Y %H:%M:%S"),
        ("en", "us") => ("%m/%d/%Y", "%m/%d/%Y %I:%M:%S %p"),
        ("en", "gb" | "ie" | "au" | "nz" | "in" | "be") => ("%d/%m/%Y", "%d/%m/%Y %H:%M:%S"),
        ("fr", _) => ("%d/%m/%Y", "%d/%m/%Y %H:%M:%S"),
        ("de", _) => ("%d.%m.%Y", "%d.%m.%Y %H:%M:%S"),
        _ => ("%Y-%m-%d", "%Y-%m-%d %H:%M:%S"),
    };
    Locale { t, date_fmt, datetime_fmt }
}
