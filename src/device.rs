//! Background thread that owns the HID device: (re)connects, sends the
//! current command once per second while running, and reads replies.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};

use crate::protocol::{self, Payload};

pub enum Event {
    Connected(bool),
    /// The device sent a reply. `sent` is the last payload we transmitted.
    Reply { sent: Payload, received: Payload },
    Error(String),
}

/// Shared slot: `Some(payload)` while running (sent every second), `None` when stopped.
pub type Outgoing = Arc<Mutex<Option<Payload>>>;

const SEND_INTERVAL: Duration = Duration::from_secs(1);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(1);

pub fn spawn(outgoing: Outgoing, events: Sender<Event>, wake: impl Fn() + Send + 'static) {
    thread::Builder::new()
        .name("hid".into())
        .spawn(move || run(outgoing, events, wake))
        .expect("spawn hid thread");
}

fn run(outgoing: Outgoing, events: Sender<Event>, wake: impl Fn()) {
    let emit = |e: Event| {
        let _ = events.send(e);
        wake();
    };

    let mut api = match HidApi::new() {
        Ok(a) => a,
        Err(e) => {
            emit(Event::Error(e.to_string()));
            return;
        }
    };

    let mut dev: Option<HidDevice> = None;
    let mut last_try: Option<Instant> = None;
    let mut last_sent: Option<Payload> = None;
    let mut last_write: Option<Instant> = None;
    let mut buf = [0u8; 64];

    loop {
        // ---- connect -------------------------------------------------
        let Some(d) = dev.as_ref() else {
            if last_try.map_or(true, |t| t.elapsed() >= RECONNECT_INTERVAL) {
                last_try = Some(Instant::now());
                if let Some(d) = open(&mut api) {
                    dev = Some(d);
                    last_write = None;
                    emit(Event::Connected(true));
                }
            }
            thread::sleep(Duration::from_millis(100));
            continue;
        };

        let mut lost = false;

        // ---- send ----------------------------------------------------
        let pending = *outgoing.lock().unwrap();
        match pending {
            Some(p) => {
                let changed = last_sent != Some(p);
                if changed || last_write.map_or(true, |t| t.elapsed() >= SEND_INTERVAL) {
                    match d.write(&protocol::to_report(&p)) {
                        Ok(_) => {
                            last_sent = Some(p);
                            last_write = Some(Instant::now());
                        }
                        Err(e) => {
                            emit(Event::Error(e.to_string()));
                            lost = true;
                        }
                    }
                }
            }
            None => last_write = None,
        }

        // ---- receive -------------------------------------------------
        if !lost {
            match d.read_timeout(&mut buf, 100) {
                Ok(0) => {}
                Ok(n) => {
                    if let (Some(received), Some(sent)) =
                        (protocol::extract_payload(&buf[..n]), last_sent)
                    {
                        emit(Event::Reply { sent, received });
                    }
                }
                Err(_) => lost = true,
            }
        }

        if lost {
            dev = None;
            emit(Event::Connected(false));
        }
    }
}

fn open(api: &mut HidApi) -> Option<HidDevice> {
    if api.refresh_devices().is_err() {
        return None;
    }
    api.device_list()
        .filter(|i| i.vendor_id() == protocol::VENDOR_ID && i.product_id() == protocol::PRODUCT_ID)
        .find_map(|i| i.open_device(api).ok())
}
