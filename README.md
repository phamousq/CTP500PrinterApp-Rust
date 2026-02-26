# CTP500 Printer App (Rust)

A native macOS Bluetooth LE client for the **Core Innovations CTP500** thermal printer, written in Rust. This is a port of the [original Python app](https://thirtythreedown.com/2025/11/02/pc-app-for-walmart-thermal-printer/).

Supports text printing (rendered to bitmap via Menlo font) and image printing via the ESC/POS raster protocol over BLE.

## Features

- Scan and connect to the CTP500 printer over Bluetooth LE
- Print text — word-wrapped and rendered at 384px width
- Print images — PNG, JPG, JPEG, BMP (auto-scaled/padded to 384px)
- Battery level indicator
- Activity log with timestamps
- Native macOS app bundle (arm64)

## Requirements

- macOS 12 or later
- Apple Silicon Mac (arm64)
- Rust toolchain (`cargo`)
- CTP500 printer powered on and in pairing mode

## Build

### Binary only

```bash
cargo build --release
```

The binary will be at `target/release/ctp500`.

### macOS .app bundle

```bash
./build-app.sh
```

This compiles a release binary and packages it into `CTP500 Printer.app`. Open it with:

```bash
open "CTP500 Printer.app"
```

Or double-click it in Finder.

> **Note:** On first Bluetooth use, macOS will show a permission prompt. If you previously denied it, go to System Settings → Privacy & Security → Bluetooth to re-enable it.

### Rebuild after code changes

```bash
cargo build --release
cp target/release/ctp500 "CTP500 Printer.app/Contents/MacOS/ctp500"
open "CTP500 Printer.app"
```

Or just re-run `./build-app.sh`.

If macOS shows "damaged" or "can't be opened" after updating the binary, clear the quarantine flag:

```bash
xattr -cr "CTP500 Printer.app"
```

## Project Structure

```
src/
├── main.rs        # Entry point — spawns Tokio thread, launches eframe window
├── app.rs         # egui UI: Bluetooth, text, image, and log sections
├── ble.rs         # BLE scan, connect, notify, and chunked write via btleplug
├── printer.rs     # Print sequence: ESC @ → start → image data → end
├── escpos.rs      # ESC/POS GS v 0 raster encoding
├── text_render.rs # Word-wrap and Menlo font rasterization to bitmap
└── types.rs       # Shared enums (BleCommand, AppEvent), constants, UUIDs
```

## Architecture

The app uses the same dual-thread model as the original Python app:

```
Main Thread (egui/eframe)        Tokio Thread (btleplug)
─────────────────────────        ──────────────────────────
[cmd_tx] ── BleCommand ──>   [cmd_rx]   receives UI commands
[evt_rx] <── AppEvent ─────  [evt_tx]   sends printer events
```

- UI sends commands (`ScanAndConnect`, `PrintImage`, etc.) via an `mpsc` channel
- BLE thread sends events (`Connected`, `BatteryLevel`, `Log`, etc.) back via a second channel
- `egui`'s `update()` drains the event channel each frame with `try_recv()`

## Dependencies

| Crate | Purpose |
|---|---|
| `eframe` + `egui` | Native GUI |
| `btleplug` | Bluetooth LE (wraps CoreBluetooth on macOS) |
| `tokio` | Async runtime for BLE operations |
| `image` | Image loading, scaling, and format conversion |
| `ab_glyph` + `imageproc` | Font loading and text rasterization |
| `rfd` | Native macOS file picker |
| `regex` | Printer name matching and battery parsing |
| `chrono` | Timestamps in activity log |

## BLE Protocol

- **Service:** ISSC Transparent UART (`49535343-fe7d-4ae5-8fa9-9fafd205e455`)
- **Write characteristic:** `49535343-8841-43f4-a8d4-ecbe34729bb3`
- **Notify characteristic:** `49535343-1e4d-4bd9-ba61-23c647249616`
- **Printer name pattern:** `S (Pink|Blue|White|Black) Printer`

### Print sequence

```
ESC @         \x1b\x40              Initialize printer
              \x1d\x49\xf0\x19      Start print sequence
GS v 0        <header + pixel data> ESC/POS raster image
              \x0a\x0a\x0a\x9a      End print sequence
```

Image data is sent in 182-byte chunks using write-with-response for flow control.

## Credits

Original Python reverse engineering and protocol documentation by [Mel at ThirtyThreeDown Studio](https://thirtythreedown.com/2025/11/02/pc-app-for-walmart-thermal-printer/), with shout-outs to Bitflip, Tsathoggualware, Reid, and others whose research made the original possible.
