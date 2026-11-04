//! Diagnostic tool: prints the raw USB traffic with the bracelet dongle.
//!
//!   cargo run --example probe                  # list device + listen 10 s, send nothing
//!   cargo run --example probe -- write 1 1 1   # send "write" (green/blue/red minutes) every second for 10 s
//!   cargo run --example probe -- close         # send "close" every second for 10 s
//!   SECS=30 cargo run --example probe -- write 2 2 2

#[path = "../src/protocol.rs"]
mod protocol;

use std::time::{Duration, Instant};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(" ")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let num = |i: usize| args.get(i).and_then(|s| s.parse::<u16>().ok()).unwrap_or(1);
    let payload = match args.first().map(String::as_str) {
        Some("write") => Some(protocol::build_payload(protocol::CMD_WRITE, num(1), num(2), num(3))),
        Some("close") => Some(protocol::build_payload(protocol::CMD_CLOSE, 0, 0, 0)),
        _ => None,
    };
    let secs: u64 = std::env::var("SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(10);

    let api = hidapi::HidApi::new().expect("hidapi init");
    let mut found = false;
    for d in api.device_list().filter(|d| d.vendor_id() == protocol::VENDOR_ID && d.product_id() == protocol::PRODUCT_ID) {
        found = true;
        println!(
            "found {:04X}:{:04X} manufacturer={:?} product={:?} serial={:?} usage_page={:#06X} usage={:#06X}\n  path={:?}",
            d.vendor_id(), d.product_id(), d.manufacturer_string(), d.product_string(),
            d.serial_number(), d.usage_page(), d.usage(), d.path()
        );
    }
    if !found {
        eprintln!("device {:04X}:{:04X} not found", protocol::VENDOR_ID, protocol::PRODUCT_ID);
        std::process::exit(1);
    }
    let dev = api.open(protocol::VENDOR_ID, protocol::PRODUCT_ID).expect("open device");

    let start = Instant::now();
    let mut last_write: Option<Instant> = None;
    let mut buf = [0u8; 64];
    while start.elapsed() < Duration::from_secs(secs) {
        if let Some(p) = payload {
            if last_write.map_or(true, |t| t.elapsed() >= Duration::from_secs(1)) {
                let r = protocol::to_report(&p);
                match dev.write(&r) {
                    Ok(n) => println!("{:6.2}s  TX {n:2}B  {}", start.elapsed().as_secs_f32(), hex(&r)),
                    Err(e) => println!("{:6.2}s  TX error: {e}", start.elapsed().as_secs_f32()),
                }
                last_write = Some(Instant::now());
            }
        }
        match dev.read_timeout(&mut buf, 100) {
            Ok(0) => {}
            Ok(n) => {
                let p = protocol::extract_payload(&buf[..n]);
                let note = match (p, payload) {
                    (Some(rx), Some(tx)) if rx == tx => "  <- echo matches (bracelet programmed)".to_string(),
                    (Some(rx), _) => format!("  <- decoded {:?}", protocol::decode(&rx)),
                    (None, _) => String::new(),
                };
                println!("{:6.2}s  RX {n:2}B  {}{note}", start.elapsed().as_secs_f32(), hex(&buf[..n]));
            }
            Err(e) => {
                println!("read error: {e}");
                break;
            }
        }
    }
}
