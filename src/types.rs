use image::DynamicImage;
use std::sync::OnceLock;
use regex::Regex;
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};

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

// ── Font choices available to the user ────────────────────────────────────────

/// A monospace font available for text printing.
pub struct FontChoice {
    /// Display label shown in the selector.
    pub label: &'static str,
    /// Absolute path to the font file on disk (loaded by ab_glyph + WebView @font-face).
    pub path: &'static str,
    /// CSS font-family value used in the textarea (must match the @font-face family name).
    pub css_family: &'static str,
}

/// All monospace fonts offered in the UI, in display order.
pub const FONT_CHOICES: &[FontChoice] = &[
    FontChoice { label: "Menlo",          path: "/System/Library/Fonts/Menlo.ttc",                              css_family: "MenloPrinter" },
    FontChoice { label: "Monaco",         path: "/System/Library/Fonts/Monaco.ttf",                             css_family: "MonacoPrinter" },
    FontChoice { label: "SF Mono",        path: "/System/Library/Fonts/SFNSMono.ttf",                           css_family: "SFMonoPrinter" },
    FontChoice { label: "PT Mono",        path: "/System/Library/Fonts/PTMono.ttc",                             css_family: "PTMonoPrinter" },
    FontChoice { label: "Courier New",    path: "/System/Library/Fonts/Supplemental/Courier New.ttf",           css_family: "CourierNewPrinter" },
    FontChoice { label: "JetBrains Mono", path: "/Users/quintonpham/Library/Fonts/JetBrainsMonoNerdFont-Regular.ttf", css_family: "JetBrainsMonoPrinter" },
    FontChoice { label: "Fira Code",      path: "/Users/quintonpham/Library/Fonts/FiraCodeNerdFont-Regular.ttf",     css_family: "FiraCodePrinter" },
];

/// Compute the number of characters that fit across PRINTER_WIDTH pixels for
/// a given font file and point size.  Uses the same ab_glyph `h_advance` path
/// as `text_render::get_wrapped_text` so the textarea width exactly matches
/// what will be printed.
pub fn chars_per_line(font_path: &str, font_size: f32) -> u32 {
    let font_data = match std::fs::read(font_path) {
        Ok(d) => d,
        Err(_) => return 21, // graceful fallback
    };
    let font = match FontVec::try_from_vec(font_data) {
        Ok(f) => f,
        Err(_) => return 21,
    };
    let scale = PxScale::from(font_size);
    let scaled = font.as_scaled(scale);
    // Use '0' (the reference glyph for the CSS `ch` unit) as the representative width
    let glyph_id = scaled.glyph_id('0');
    let advance = scaled.h_advance(glyph_id);
    if advance <= 0.0 {
        return 21;
    }
    (PRINTER_WIDTH as f32 / advance).floor() as u32
}

/// Commands sent from the UI thread to the BLE thread.
#[derive(Debug)]
pub enum BleCommand {
    ScanAndConnect,
    Disconnect,
    PrintImage(DynamicImage),
    /// font_path: absolute path to the .ttf/.ttc file used by ab_glyph
    /// font_size: point size used when rendering
    PrintText { text: String, font_path: String, font_size: f32 },
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
