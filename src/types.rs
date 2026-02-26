use image::DynamicImage;
use std::sync::OnceLock;
use regex::Regex;

// BLE UUIDs
pub const WRITE_CHAR_UUID: &str = "49535343-8841-43f4-a8d4-ecbe34729bb3";
pub const NOTIFY_CHAR_UUID: &str = "49535343-1e4d-4bd9-ba61-23c647249616";

// Printer configuration
pub const PRINTER_WIDTH: u32 = 384;
pub const CHUNK_SIZE: usize = 182; // Conservative MTU-3 on macOS (btleplug doesn't expose MTU)

// LiPo voltage range for the CTP500 battery
pub const BATT_MIN_MV: u32 = 3300; // 0%
pub const BATT_MAX_MV: u32 = 4200; // 100%

// Printer name regex: matches "S Blue Printer", "S Pink Printer", etc.
static PRINTER_NAME_RE: OnceLock<Regex> = OnceLock::new();
pub fn printer_name_regex() -> &'static Regex {
    PRINTER_NAME_RE.get_or_init(|| {
        Regex::new(r"(?i)S\s+(Pink|Blue|White|Black)\s+Printer").unwrap()
    })
}

// Battery voltage regex: matches "VOLT=4000mv"
static BATTERY_RE: OnceLock<Regex> = OnceLock::new();
pub fn battery_regex() -> &'static Regex {
    BATTERY_RE.get_or_init(|| {
        Regex::new(r"VOLT=(\d+)mv").unwrap()
    })
}

/// Parse battery percentage from printer status response.
/// Response format: "HV=V1.0A,SV=V1.01,VOLT=4000mv,DPI=384,"
/// Returns 0-100 or None if not found.
pub fn parse_battery(data: &[u8]) -> Option<u8> {
    let text = String::from_utf8_lossy(data);
    let caps = battery_regex().captures(&text)?;
    let mv: u32 = caps[1].parse().ok()?;
    let pct = ((mv.saturating_sub(BATT_MIN_MV)) as f64
        / (BATT_MAX_MV - BATT_MIN_MV) as f64
        * 100.0) as i32;
    Some(pct.clamp(0, 100) as u8)
}

/// Commands sent from the UI thread to the BLE thread.
#[derive(Debug)]
pub enum BleCommand {
    ScanAndConnect,
    Disconnect,
    PrintImage(DynamicImage),
    PrintText(String),
}

/// Events sent from the BLE thread back to the UI thread.
#[derive(Debug)]
pub enum AppEvent {
    Log(String),
    Connected,
    Disconnected,
    BatteryLevel(u8),
    PrintProgress { sent: usize, total: usize },
    Error(String),
    ScanStarted,
    PrintComplete,
}
