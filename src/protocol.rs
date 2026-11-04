//! Wire protocol of the timer base station, recovered from the original
//! "Timer Controller V1.2" (HidLib.dll / frmMain).
//!
//! The device is a plain USB HID device (no driver needed) with
//! VID 0x1911 / PID 0xD168 and 32-byte input/output reports (report id 0).
//!
//! Output report (33 bytes incl. report id):
//!   [0]      0x00 report id
//!   [1]      0x10 payload length (16)
//!   [2..18]  payload
//!   [18..33] zero padding
//!
//! Payload (16 bytes):
//!   A5 5A | cmd | green_hi green_lo | yellow_hi yellow_lo | red_hi red_lo | wt | 00 00 00 00 00 | checksum
//!   cmd      0x00 = write (program) timer, 0xFF = close timer
//!   minutes  big-endian u16 per colour
//!   wt       always 0 in the original software
//!   checksum (0xFF + cmd + all six minute bytes + wt) & 0xFF
//!
//! The device echoes the payload back (same layout) when a timer has been
//! programmed. A matching echo of a write command creates a history record.

pub const VENDOR_ID: u16 = 0x1911;
pub const PRODUCT_ID: u16 = 0xD168;

pub const CMD_WRITE: u8 = 0x00;
pub const CMD_CLOSE: u8 = 0xFF;

pub const PAYLOAD_LEN: usize = 16;
pub const REPORT_LEN: usize = 33;

pub type Payload = [u8; PAYLOAD_LEN];

/// Build the 16-byte command payload.
pub fn build_payload(cmd: u8, green: u16, yellow: u16, red: u16) -> Payload {
    let wt: u8 = 0;
    let mut p = [0u8; PAYLOAD_LEN];
    p[0] = 0xA5;
    p[1] = 0x5A;
    p[2] = cmd;
    p[3..5].copy_from_slice(&green.to_be_bytes());
    p[5..7].copy_from_slice(&yellow.to_be_bytes());
    p[7..9].copy_from_slice(&red.to_be_bytes());
    p[9] = wt;
    // p[10..15] stay zero
    let sum: u32 = 0xFF + p[2..10].iter().map(|&b| b as u32).sum::<u32>();
    p[15] = (sum & 0xFF) as u8;
    p
}

/// Payload used to switch a bracelet off: a write with 0 minutes for every colour.
///
/// The original "Close timer" command (`CMD_CLOSE`, 0xFF) is never acknowledged
/// by the bracelets (tested on hardware), so the app can't tell whether it
/// worked. A 0/0/0 write *is* acknowledged, so it is used instead.
pub fn turn_off_payload() -> Payload {
    build_payload(CMD_WRITE, 0, 0, 0)
}

/// True if this (sent) payload switches bracelets off rather than programming them.
pub fn is_turn_off(p: &Payload) -> bool {
    let (cmd, g, b, r) = decode(p);
    cmd == CMD_CLOSE || (g, b, r) == (0, 0, 0)
}

/// Wrap a payload into a full output report (with report id 0 in front).
pub fn to_report(payload: &Payload) -> [u8; REPORT_LEN] {
    let mut r = [0u8; REPORT_LEN];
    r[0] = 0x00;
    r[1] = PAYLOAD_LEN as u8;
    r[2..2 + PAYLOAD_LEN].copy_from_slice(payload);
    r
}

/// Extract the 16-byte payload from an input report.
///
/// hidapi strips the report id on all platforms for unnumbered reports, so
/// normally the buffer is `[len, A5, 5A, ...]`; we also accept a buffer that
/// still has the report id in front.
pub fn extract_payload(buf: &[u8]) -> Option<Payload> {
    for offset in [1usize, 2] {
        if buf.len() >= offset + PAYLOAD_LEN && buf[offset] == 0xA5 && buf[offset + 1] == 0x5A {
            let mut p = [0u8; PAYLOAD_LEN];
            p.copy_from_slice(&buf[offset..offset + PAYLOAD_LEN]);
            return Some(p);
        }
    }
    None
}

/// Decoded command: (cmd, green, yellow, red) minutes.
pub fn decode(p: &Payload) -> (u8, u16, u16, u16) {
    (
        p[2],
        u16::from_be_bytes([p[3], p[4]]),
        u16::from_be_bytes([p[5], p[6]]),
        u16::from_be_bytes([p[7], p[8]]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference values computed by hand from the original VB code:
    /// senddata = "A55A" + cmd + green + yellow + red + wtime + "0000000000" + cs
    #[test]
    fn matches_original_hex_string() {
        let p = build_payload(CMD_WRITE, 1, 1, 1);
        assert_eq!(hex(&p), "A55A0000010001000100000000000002");

        let p = build_payload(CMD_WRITE, 30, 5, 10);
        // 255 + 0 + 0x1E + 0x05 + 0x0A = 0x12C -> 2C
        assert_eq!(hex(&p), "A55A00001E0005000A0000000000002C");

        let p = build_payload(CMD_CLOSE, 999, 999, 999);
        // 255 + 255 + 3*(0x03 + 0xE7) = 0x4BC -> BC
        assert_eq!(hex(&p), "A55AFF03E703E703E7000000000000BC");
    }

    #[test]
    fn report_layout() {
        let p = build_payload(CMD_WRITE, 1, 2, 3);
        let r = to_report(&p);
        assert_eq!(r[0], 0);
        assert_eq!(r[1], 16);
        assert_eq!(&r[2..18], &p);
        assert!(r[18..].iter().all(|&b| b == 0));
    }

    #[test]
    fn extract_and_decode() {
        let p = build_payload(CMD_WRITE, 12, 3, 45);
        let r = to_report(&p);
        // without report id (normal hidapi behaviour)
        assert_eq!(extract_payload(&r[1..]), Some(p));
        // with report id
        assert_eq!(extract_payload(&r), Some(p));
        assert_eq!(decode(&p), (CMD_WRITE, 12, 3, 45));
        assert_eq!(extract_payload(&[0u8; 32]), None);
    }

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02X}")).collect()
    }
}
