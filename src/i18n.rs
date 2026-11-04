//! English + Dutch UI text. The language and date format follow the system
//! locale; set TIMER_LANG=en or TIMER_LANG=nl to override.

pub struct Texts {
    pub title: &'static str,
    pub send_command: &'static str,
    pub green: &'static str,
    pub blue: &'static str,
    pub red: &'static str,
    pub minutes: &'static str,
    pub write_timer: &'static str,
    pub close_timer: &'static str,
    pub start: &'static str,
    pub stop: &'static str,
    pub online: &'static str,
    pub summary: &'static str,
    pub from: &'static str,
    pub to: &'static str,
    pub check_total: &'static str,
    pub total: &'static str,
    pub delete: &'static str,
    pub communication: &'static str,
    pub device: &'static str,
    pub connected: &'static str,
    pub disconnected: &'static str,
    pub sending: &'static str,
    pub device_offline: &'static str,
    pub checksum_error: &'static str,
    pub no_delete_today: &'static str,
    pub range_invalid: &'static str,
    /// `{n}` is replaced by the number of records.
    pub confirm_delete: &'static str,
    pub deleted: &'static str,
    pub notice: &'static str,
    pub ok: &'static str,
    pub cancel: &'static str,
    pub db_error: &'static str,
    pub device_error: &'static str,
    pub last_programmed: &'static str,
}

pub const EN: Texts = Texts {
    title: "Timer Controller",
    send_command: "Send command",
    green: "Green",
    blue: "Blue",
    red: "Red",
    minutes: "min",
    write_timer: "Write timer",
    close_timer: "Close timer",
    start: "Start",
    stop: "Stop",
    online: "Online quantities",
    summary: "Sum up / delete",
    from: "Start",
    to: "End",
    check_total: "Check total",
    total: "Total",
    delete: "Delete records",
    communication: "Communication",
    device: "Device",
    connected: "Connected",
    disconnected: "Not connected",
    sending: "Sending…",
    device_offline: "The device is offline. Please connect the device.",
    checksum_error: "Checksum error: the device reply does not match the command that was sent.",
    no_delete_today: "Deleting today's records is not allowed. Please choose an earlier date.",
    range_invalid: "The start must be before the end.",
    confirm_delete: "Delete {n} records?",
    deleted: "Deleted {n} records.",
    notice: "Notice",
    ok: "OK",
    cancel: "Cancel",
    db_error: "Database error",
    device_error: "Device error",
    last_programmed: "Last timer programmed",
};

pub const NL: Texts = Texts {
    title: "Timer Controller",
    send_command: "Opdracht verzenden",
    green: "Groen",
    blue: "Blauw",
    red: "Rood",
    minutes: "min",
    write_timer: "Timer schrijven",
    close_timer: "Timer sluiten",
    start: "Starten",
    stop: "Stoppen",
    online: "Actieve timers",
    summary: "Overzicht / verwijderen",
    from: "Van",
    to: "Tot",
    check_total: "Totaal tellen",
    total: "Totaal",
    delete: "Records verwijderen",
    communication: "Communicatie",
    device: "Apparaat",
    connected: "Verbonden",
    disconnected: "Niet verbonden",
    sending: "Bezig met verzenden…",
    device_offline: "Het apparaat is offline. Sluit het apparaat aan.",
    checksum_error: "Controlefout: het antwoord van het apparaat komt niet overeen met de verzonden opdracht.",
    no_delete_today: "Records van vandaag verwijderen is niet toegestaan. Kies een eerdere datum.",
    range_invalid: "Het begin moet vóór het einde liggen.",
    confirm_delete: "{n} records verwijderen?",
    deleted: "{n} records verwijderd.",
    notice: "Melding",
    ok: "OK",
    cancel: "Annuleren",
    db_error: "Databasefout",
    device_error: "Apparaatfout",
    last_programmed: "Laatst geprogrammeerde timer",
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
