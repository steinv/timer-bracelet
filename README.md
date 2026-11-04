# Timer Bracelet Controller

A cross-platform (Windows, macOS, Linux) controller for USB **timer bracelets**: wristbands that
light up **green → blue → red** for a set number of minutes each. It replaces the original
Windows-only *Timer Controller V1.2* with a Rust app.

- Runs on **Windows, macOS (Intel & Apple Silicon) and Linux**
- **English and Dutch** interface, chosen automatically from the system language. Dates use the
  system's date format, and the light/dark theme follows the system.
- Shows how many bracelets are currently green, blue or red
- Keeps a history of programmed bracelets in a local SQLite database (count or clean up by date)

## Download

Get the latest build from the [**Releases**](https://github.com/steinv/timer-bracelet/releases) page:

| Platform | File | Notes |
|----------|------|-------|
| Windows  | `TimerController-windows-x64.zip` | Unzip and run `TimerController.exe`. |
| macOS    | `TimerController-macos-universal.zip` | Unzip and move *Timer Controller.app* to Applications. The app isn't notarized, so the first time, right-click it → **Open** (or run `xattr -dr com.apple.quarantine "/Applications/Timer Controller.app"`). |
| Linux    | `TimerController-linux-x64.tar.gz` | Extract, install the udev rule (below), then run `./timer-controller`. |

### Linux: USB permissions

Without this rule, only root can access the dongle:

```sh
sudo cp 99-timer-controller.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Then unplug the dongle and plug it back in.

## Usage

1. Plug in the USB dongle. The status bar at the bottom shows **Connected**.
2. Choose **Program bracelets** or **Turn bracelets off** at the top.
3. When programming, set the **green / blue / red** minutes (0–999).
4. Press the round **power button**. The app sends the command every second until you press it
   again. The settings are locked while it is on.
5. **Activate each bracelet with the magnet:** hold the magnet against it until the light
   flickers, then hold the bracelet next to the dongle. When the bracelet confirms,
   *✔ Bracelet programmed* (or *turned off*) appears and the session counter goes up.

**Active bracelets** shows how many bracelets are green, blue or red right now. This is
calculated from the history, not read from the bracelets. A bracelet that is turned off no longer
counts. The dongle doesn't say *which* bracelet was turned off, so the oldest running one is
marked as ended.

**History** updates live. Pick a preset (Today, Yesterday, Last 7 days, Everything before today)
or your own period to see how many bracelets were programmed and when. Records from today can't be
deleted.

### Troubleshooting

- **Bracelet doesn't respond.** A bracelet only listens right after it is activated with the
  magnet (its light flickers), even when its light is already on. Activate it, then hold it next to
  the dongle. It can take a few seconds to answer.
- **"Not connected".** Check the cable. On Linux, check the udev rule above. The app reconnects
  automatically when the dongle is plugged back in.
- **Language.** Set `TIMER_LANG=nl` or `TIMER_LANG=en` to override the system language.

### Data location

| OS      | Database file                                               |
|---------|-------------------------------------------------------------|
| Windows | `%APPDATA%\TimerController\data\timer.db`                    |
| macOS   | `~/Library/Application Support/TimerController/timer.db`     |
| Linux   | `~/.local/share/timercontroller/timer.db`                     |

History from the old program's Access database (`DSdb.mdb`) is not imported.

## Building from source

Install Rust from <https://rustup.rs>, then:

```sh
cargo build --release      # -> target/release/timer-controller
cargo test                 # protocol + database tests
```

Prerequisites:

- **Windows**: Visual Studio Build Tools with *Desktop development with C++*
- **macOS**: `xcode-select --install`
- **Debian/Ubuntu**: `sudo apt install build-essential pkg-config libudev-dev libxkbcommon-dev`
- **Fedora**: `sudo dnf install gcc pkgconf-pkg-config systemd-devel libxkbcommon-devel`

### Diagnostics

`examples/probe.rs` prints the raw USB traffic. It's useful when a bracelet doesn't respond:

```sh
cargo run --example probe                  # show the dongle and listen for 10 s
cargo run --example probe -- write 1 1 1   # program: 1 min green / blue / red
SECS=30 cargo run --example probe -- close
```

## Releasing

Pushing a version tag builds Windows, macOS and Linux packages and publishes a GitHub release:

```sh
git tag v2.0.0
git push origin v2.0.0
```

Every push to `main` is built and tested on all three platforms (`.github/workflows/ci.yml`).

## Protocol

The dongle is a WCH "HID To Serial" bridge (USB VID `1911`, PID `D168`, vendor usage page
`0xFFA0`) with 32-byte HID reports, report ID 0. This was recovered from the original software and
verified against real hardware.

Each output report holds a length byte (`0x10`) followed by this 16-byte payload:

```
A5 5A | cmd | G_hi G_lo | B_hi B_lo | R_hi R_lo | 00 | 00 00 00 00 00 | checksum

cmd       00 = write timer, FF = close timer
G/B/R     minutes per colour, big-endian u16
checksum  (0xFF + cmd + the six minute bytes + 0) & 0xFF
```

When a bracelet accepts a write command, the dongle sends back the same payload
(`10 A5 5A …`). The app treats that echo as one programmed bracelet. It is green from that moment
for G minutes, then blue for B minutes, then red for R minutes.

**Turning bracelets off:** the original *Close timer* command (`cmd FF`) is never acknowledged by
the bracelets. The app sends a write with 0/0/0 minutes instead, which *is* acknowledged.

## License

[MIT](LICENSE)

The app embeds the [DejaVu Sans](https://dejavu-fonts.github.io/) font as a fallback for symbols. Its license
is in [`assets/fonts/DejaVu-LICENSE.txt`](assets/fonts/DejaVu-LICENSE.txt).
