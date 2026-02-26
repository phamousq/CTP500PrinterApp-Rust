use dioxus::prelude::*;
use image::DynamicImage;

use crate::types::{AppEvent, BleCommand, FONT_CHOICES, chars_per_line};

// ── Shared state passed into the app via context ──────────────────────────────

pub struct AppState {
    pub cmd_tx: tokio::sync::mpsc::Sender<BleCommand>,
    pub evt_rx: tokio::sync::mpsc::Receiver<AppEvent>,
}

// ── Root component ────────────────────────────────────────────────────────────

#[component]
pub fn App() -> Element {
    // ── Reactive signals ──────────────────────────────────────────────────────
    let mut connected = use_signal(|| false);
    let mut scanning = use_signal(|| false);
    let mut battery_pct: Signal<Option<u8>> = use_signal(|| None);
    let mut log_entries: Signal<Vec<String>> = use_signal(Vec::new);
    let mut text_input = use_signal(String::new);
    let mut current_image: Signal<Option<DynamicImage>> = use_signal(|| None);
    // Base64-encoded PNG thumbnail for the WebView <img> tag
    let mut image_preview_b64: Signal<Option<String>> = use_signal(|| None);
    let mut printing = use_signal(|| false);
    let mut print_progress: Signal<Option<(usize, usize)>> = use_signal(|| None);
    let mut last_error: Signal<Option<String>> = use_signal(|| None);

    // ── Font / size signals ───────────────────────────────────────────────────
    // font_idx: index into FONT_CHOICES; font_size_px: point size for rendering
    let mut font_idx = use_signal(|| 0usize);
    let mut font_size_px = use_signal(|| 28u32);

    // ── Retrieve channels from context ────────────────────────────────────────
    let state = use_context::<std::sync::Arc<tokio::sync::Mutex<AppState>>>();

    // ── BLE event pump: drains AppEvent channel and writes to signals ─────────
    // spawn_forever keeps this alive for the lifetime of the app.
    use_hook(|| {
        let state = state.clone();
        spawn_forever(async move {
            loop {
                let event = {
                    let mut s = state.lock().await;
                    s.evt_rx.recv().await
                };
                match event {
                    Some(AppEvent::Log(msg)) => {
                        let ts = chrono::Local::now().format("%H:%M:%S").to_string();
                        let entry = format!("[{}] {}", ts, msg);
                        log_entries.with_mut(|v| {
                            v.push(entry);
                            if v.len() > 200 { v.drain(..50); }
                        });
                    }
                    Some(AppEvent::Connected) => {
                        connected.set(true);
                        scanning.set(false);
                        let ts = chrono::Local::now().format("%H:%M:%S").to_string();
                        log_entries.with_mut(|v| v.push(format!("[{}] Connected", ts)));
                    }
                    Some(AppEvent::Disconnected) => {
                        connected.set(false);
                        scanning.set(false);
                        battery_pct.set(None);
                        printing.set(false);
                        print_progress.set(None);
                    }
                    Some(AppEvent::BatteryLevel(pct)) => {
                        battery_pct.set(Some(pct));
                    }
                    Some(AppEvent::ScanStarted) => {
                        scanning.set(true);
                    }
                    Some(AppEvent::PrintProgress { sent, total }) => {
                        print_progress.set(Some((sent, total)));
                        printing.set(true);
                    }
                    Some(AppEvent::PrintComplete) => {
                        printing.set(false);
                        print_progress.set(None);
                        let ts = chrono::Local::now().format("%H:%M:%S").to_string();
                        log_entries.with_mut(|v| v.push(format!("[{}] Print complete", ts)));
                    }
                    Some(AppEvent::Error(e)) => {
                        last_error.set(Some(e.clone()));
                        let ts = chrono::Local::now().format("%H:%M:%S").to_string();
                        log_entries.with_mut(|v| v.push(format!("[{}] Error: {}", ts, e)));
                        printing.set(false);
                        scanning.set(false);
                    }
                    None => break, // channel closed
                }
            }
        });
    });

    // ── Derived display values ────────────────────────────────────────────────
    let status_text = if *scanning.read() {
        "⟳ Scanning..."
    } else if *connected.read() {
        "● Connected"
    } else {
        "● Disconnected"
    };

    let status_color = if *scanning.read() {
        "#0066cc"
    } else if *connected.read() {
        "#00aa00"
    } else {
        "#cc0000"
    };

    let battery_display = (*battery_pct.read()).map(|pct| {
        let color = if pct > 50 { "#00aa00" } else if pct > 20 { "#cc7700" } else { "#cc0000" };
        (pct, color)
    });

    let can_print_text = *connected.read()
        && !text_input.read().trim().is_empty()
        && !*printing.read();

    let can_print_image = *connected.read()
        && current_image.read().is_some()
        && !*printing.read();

    let progress_display = *print_progress.read();

    // ── Font / size derived values ────────────────────────────────────────────
    let idx = *font_idx.read();
    let size = *font_size_px.read();
    let font = &FONT_CHOICES[idx];
    let font_path_str = font.path;
    let css_family = font.css_family;
    // Compute chars that fit the 384px printer width at the current size
    let cols = chars_per_line(font_path_str, size as f32);
    // Inline style for the textarea: dynamic font-family, font-size, and width
    let textarea_style = format!(
        "font-family: '{}', monospace; font-size: {}px; width: {}ch;",
        css_family, size, cols
    );

    // ── Clones for event handlers ─────────────────────────────────────────────
    let state_ble = state.clone();
    let state_ble2 = state.clone();
    let state_print_text = state.clone();
    let state_print_image = state.clone();

    rsx! {
        style { {STYLES} }

        div { class: "container",

            // ── Bluetooth section ─────────────────────────────────────────────
            section { class: "card",
                h2 { class: "section-title", "Bluetooth Tools" }

                div { class: "btn-row",
                    if !*connected.read() {
                        button {
                            class: "btn btn-primary",
                            disabled: *scanning.read(),
                            onclick: move |_| {
                                let state = state_ble.clone();
                                scanning.set(true);
                                last_error.set(None);
                                spawn(async move {
                                    let s = state.lock().await;
                                    s.cmd_tx.send(BleCommand::ScanAndConnect).await.ok();
                                });
                            },
                            if *scanning.read() { "Scanning..." } else { "Scan & Connect" }
                        }
                    } else {
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| {
                                let state = state_ble2.clone();
                                spawn(async move {
                                    let s = state.lock().await;
                                    s.cmd_tx.send(BleCommand::Disconnect).await.ok();
                                });
                            },
                            "Disconnect"
                        }
                    }
                }

                p {
                    class: "status-text",
                    style: "color: {status_color}",
                    "{status_text}"
                }

                if let Some((pct, color)) = battery_display {
                    p {
                        class: "battery-text",
                        style: "color: {color}",
                        "Battery: {pct}%"
                    }
                }

                if let Some(ref err) = *last_error.read() {
                    p { class: "error-text", "Error: {err}" }
                }
            }

            // ── Text tools section ────────────────────────────────────────────
            section { class: "card",
                h2 { class: "section-title", "Text Tools" }

                // Font selector
                div { class: "control-row",
                    label { class: "control-label", r#for: "font-select", "Font" }
                    select {
                        id: "font-select",
                        class: "control-select",
                        value: "{idx}",
                        onchange: move |e| {
                            if let Ok(v) = e.value().parse::<usize>() {
                                font_idx.set(v);
                            }
                        },
                        for (i, fc) in FONT_CHOICES.iter().enumerate() {
                            option { value: "{i}", selected: i == idx, "{fc.label}" }
                        }
                    }
                }

                // Font size slider
                div { class: "control-row",
                    label { class: "control-label", r#for: "font-size-slider",
                        "Size: {size}px  ({cols} chars/line)"
                    }
                    input {
                        id: "font-size-slider",
                        class: "control-slider",
                        r#type: "range",
                        min: "12",
                        max: "48",
                        step: "1",
                        value: "{size}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<u32>() {
                                font_size_px.set(v);
                            }
                        },
                    }
                }

                // Textarea sized dynamically to match printer output
                div { class: "text-input-wrap",
                    textarea {
                        class: "text-input",
                        style: "{textarea_style}",
                        placeholder: "Type or paste\ntext to print...",
                        rows: "5",
                        value: "{text_input}",
                        oninput: move |e| text_input.set(e.value()),
                    }
                }

                button {
                    class: "btn btn-outline",
                    onclick: move |_| {
                        spawn(async move {
                            if let Some(path) = rfd::AsyncFileDialog::new()
                                .add_filter("Text files", &["txt"])
                                .add_filter("All files", &["*"])
                                .pick_file()
                                .await
                            {
                                match std::fs::read_to_string(path.path()) {
                                    Ok(content) => text_input.set(content),
                                    Err(e) => last_error.set(Some(format!("Failed to read file: {}", e))),
                                }
                            }
                        });
                    },
                    "Select a text file"
                }

                button {
                    class: "btn btn-primary",
                    disabled: !can_print_text,
                    onclick: move |_| {
                        let state = state_print_text.clone();
                        let text = text_input.read().clone();
                        let fp = FONT_CHOICES[*font_idx.read()].path.to_string();
                        let fs = *font_size_px.read() as f32;
                        printing.set(true);
                        last_error.set(None);
                        spawn(async move {
                            let s = state.lock().await;
                            s.cmd_tx.send(BleCommand::PrintText {
                                text,
                                font_path: fp,
                                font_size: fs,
                            }).await.ok();
                        });
                    },
                    "Print your text!"
                }
            }

            // ── Image tools section ───────────────────────────────────────────
            section { class: "card",
                h2 { class: "section-title", "Image Tools" }

                div { class: "image-preview",
                    if let Some(ref b64) = *image_preview_b64.read() {
                        img {
                            src: "data:image/png;base64,{b64}",
                            class: "preview-img",
                            alt: "Image preview",
                        }
                    } else {
                        div { class: "preview-placeholder", "No image loaded" }
                    }
                }

                button {
                    class: "btn btn-outline",
                    onclick: move |_| {
                        spawn(async move {
                            if let Some(file) = rfd::AsyncFileDialog::new()
                                .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
                                .add_filter("All files", &["*"])
                                .pick_file()
                                .await
                            {
                                match image::open(file.path()) {
                                    Ok(img) => {
                                        let thumb = img.thumbnail(300, 100);
                                        let mut buf = Vec::new();
                                        if thumb.write_to(
                                            &mut std::io::Cursor::new(&mut buf),
                                            image::ImageFormat::Png,
                                        ).is_ok() {
                                            use base64::Engine;
                                            let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
                                            image_preview_b64.set(Some(b64));
                                        }
                                        current_image.set(Some(img));
                                    }
                                    Err(e) => {
                                        last_error.set(Some(format!("Failed to open image: {}", e)));
                                    }
                                }
                            }
                        });
                    },
                    "Select an image file"
                }

                button {
                    class: "btn btn-primary",
                    disabled: !can_print_image,
                    onclick: move |_| {
                        let state = state_print_image.clone();
                        if let Some(img) = current_image.read().clone() {
                            printing.set(true);
                            last_error.set(None);
                            spawn(async move {
                                let s = state.lock().await;
                                s.cmd_tx.send(BleCommand::PrintImage(img)).await.ok();
                            });
                        }
                    },
                    "Print your image!"
                }

                if let Some((sent, total)) = progress_display {
                    div { class: "progress-wrap",
                        p { class: "progress-label",
                            "Sending... {sent}/{total} bytes"
                        }
                        div { class: "progress-bar-bg",
                            div {
                                class: "progress-bar-fill",
                                style: "width: {sent as f32 / total as f32 * 100.0:.1}%",
                            }
                        }
                    }
                }
            }

            // ── Activity log section ──────────────────────────────────────────
            section { class: "card",
                h2 { class: "section-title", "Activity Log" }
                div { class: "log-box",
                    id: "log-scroll",
                    for entry in log_entries.read().iter() {
                        p { class: "log-entry", "{entry}" }
                    }
                }
            }
        }

        // Auto-scroll log to bottom whenever entries change
        script {
            r#"
            (function() {{
                var el = document.getElementById('log-scroll');
                if (el) el.scrollTop = el.scrollHeight;
            }})();
            "#
        }
    }
}

// ── Embedded CSS ──────────────────────────────────────────────────────────────

const STYLES: &str = r#"
/* @font-face declarations — one per available printer font.
   The CSS family name must match what app.rs injects into the textarea style. */
@font-face {
    font-family: "MenloPrinter";
    src: url("file:///System/Library/Fonts/Menlo.ttc") format("truetype");
    font-weight: normal; font-style: normal;
}
@font-face {
    font-family: "MonacoPrinter";
    src: url("file:///System/Library/Fonts/Monaco.ttf") format("truetype");
    font-weight: normal; font-style: normal;
}
@font-face {
    font-family: "SFMonoPrinter";
    src: url("file:///System/Library/Fonts/SFNSMono.ttf") format("truetype");
    font-weight: normal; font-style: normal;
}
@font-face {
    font-family: "PTMonoPrinter";
    src: url("file:///System/Library/Fonts/PTMono.ttc") format("truetype");
    font-weight: normal; font-style: normal;
}
@font-face {
    font-family: "CourierNewPrinter";
    src: url("file:///System/Library/Fonts/Supplemental/Courier%20New.ttf") format("truetype");
    font-weight: normal; font-style: normal;
}
@font-face {
    font-family: "JetBrainsMonoPrinter";
    src: url("file:///Users/quintonpham/Library/Fonts/JetBrainsMonoNerdFont-Regular.ttf") format("truetype");
    font-weight: normal; font-style: normal;
}
@font-face {
    font-family: "FiraCodePrinter";
    src: url("file:///Users/quintonpham/Library/Fonts/FiraCodeNerdFont-Regular.ttf") format("truetype");
    font-weight: normal; font-style: normal;
}

*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

body {
    font-family: -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif;
    font-size: 14px;
    background: #f0f0f0;
    color: #1a1a1a;
    min-height: 100vh;
}

.container {
    max-width: 520px;
    margin: 0 auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
}

.card {
    background: #ffffff;
    border-radius: 10px;
    padding: 14px;
    box-shadow: 0 1px 4px rgba(0,0,0,0.10);
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.section-title {
    font-size: 13px;
    font-weight: 600;
    color: #555;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-bottom: 2px;
}

/* Buttons */
.btn {
    display: block;
    width: 100%;
    padding: 10px 16px;
    border: none;
    border-radius: 7px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: opacity 0.15s, background 0.15s;
}
.btn:disabled { opacity: 0.45; cursor: not-allowed; }
.btn-primary  { background: #0071e3; color: #fff; }
.btn-primary:hover:not(:disabled)  { background: #0064cc; }
.btn-secondary { background: #e5e5ea; color: #1a1a1a; }
.btn-secondary:hover:not(:disabled) { background: #d1d1d6; }
.btn-outline  { background: transparent; color: #0071e3;
                border: 1.5px solid #0071e3; }
.btn-outline:hover:not(:disabled)  { background: #e8f0fc; }

.btn-row { display: flex; gap: 8px; }
.btn-row .btn { flex: 1; }

/* Status */
.status-text { font-size: 13px; font-weight: 500; }
.battery-text { font-size: 13px; }
.error-text { font-size: 12px; color: #cc0000; }

/* Font / size controls */
.control-row {
    display: flex;
    align-items: center;
    gap: 10px;
}
.control-label {
    font-size: 12px;
    color: #555;
    white-space: nowrap;
    min-width: 140px;
}
.control-select {
    flex: 1;
    padding: 5px 8px;
    border: 1.5px solid #d1d1d6;
    border-radius: 6px;
    font-size: 13px;
    background: #fff;
    color: #1a1a1a;
    cursor: pointer;
}
.control-slider {
    flex: 1;
    cursor: pointer;
    accent-color: #0071e3;
}

/* Text input — width and font are set dynamically via inline style */
.text-input-wrap {
    display: flex;
    justify-content: center;
}
.text-input {
    /* width, font-family, and font-size are injected as inline style by app.rs
       so the textarea exactly mirrors what will be rendered on the 384px printer. */
    box-sizing: content-box;
    padding: 8px 10px;
    border: 1.5px solid #d1d1d6;
    border-radius: 7px;
    line-height: 1.45;
    resize: none;
    outline: none;
    transition: border-color 0.15s;
    display: block;
}
.text-input:focus { border-color: #0071e3; }

/* Image preview */
.image-preview {
    width: 100%;
    height: 110px;
    border: 1.5px solid #d1d1d6;
    border-radius: 7px;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    background: #fafafa;
}
.preview-img { max-width: 100%; max-height: 108px; object-fit: contain; }
.preview-placeholder { color: #aaa; font-size: 13px; }

/* Progress */
.progress-wrap { display: flex; flex-direction: column; gap: 4px; }
.progress-label { font-size: 12px; color: #555; }
.progress-bar-bg {
    width: 100%; height: 6px;
    background: #e5e5ea; border-radius: 3px; overflow: hidden;
}
.progress-bar-fill {
    height: 100%;
    background: #0071e3;
    border-radius: 3px;
    transition: width 0.2s;
}

/* Log */
.log-box {
    background: #1e1e1e;
    border-radius: 7px;
    padding: 8px 10px;
    height: 160px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 1px;
}
.log-entry {
    font-family: "Menlo", "Courier New", monospace;
    font-size: 11px;
    color: #d4d4d4;
    white-space: pre-wrap;
    word-break: break-all;
}
"#;
