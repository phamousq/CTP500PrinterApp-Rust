use eframe::egui;
use image::DynamicImage;
use tokio::sync::mpsc::{Sender, Receiver};

use crate::types::{AppEvent, BleCommand};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Justification {
    Left,
    Center,
    Right,
}

pub struct PrinterApp {
    // Channel handles
    cmd_tx: Sender<BleCommand>,
    evt_rx: Receiver<AppEvent>,

    // Connection state
    connected: bool,
    scanning: bool,
    battery_pct: Option<u8>,

    // Log
    log_entries: Vec<String>,

    // Text printing
    text_input: String,
    justification: Justification,

    // Image printing
    current_image: Option<DynamicImage>,
    image_preview_texture: Option<egui::TextureHandle>,

    // Print progress
    printing: bool,
    print_progress: Option<(usize, usize)>,

    // Error display
    last_error: Option<String>,
}

impl PrinterApp {
    pub fn new(cmd_tx: Sender<BleCommand>, evt_rx: Receiver<AppEvent>) -> Self {
        Self {
            cmd_tx,
            evt_rx,
            connected: false,
            scanning: false,
            battery_pct: None,
            log_entries: Vec::new(),
            text_input: String::new(),
            justification: Justification::Left,
            current_image: None,
            image_preview_texture: None,
            printing: false,
            print_progress: None,
            last_error: None,
        }
    }

    fn push_log(&mut self, message: String) {
        let ts = chrono::Local::now().format("%H:%M:%S").to_string();
        self.log_entries.push(format!("[{}] {}", ts, message));
        // Keep the log from growing unbounded
        if self.log_entries.len() > 200 {
            self.log_entries.drain(..50);
        }
    }

    fn handle_event(&mut self, event: AppEvent, ctx: &egui::Context) {
        match event {
            AppEvent::Log(msg) => self.push_log(msg),
            AppEvent::Connected => {
                self.connected = true;
                self.scanning = false;
                self.push_log("Connected".into());
            }
            AppEvent::Disconnected => {
                self.connected = false;
                self.scanning = false;
                self.battery_pct = None;
                self.printing = false;
                self.print_progress = None;
            }
            AppEvent::BatteryLevel(pct) => {
                self.battery_pct = Some(pct);
            }
            AppEvent::ScanStarted => {
                self.scanning = true;
            }
            AppEvent::PrintProgress { sent, total } => {
                self.print_progress = Some((sent, total));
                self.printing = true;
            }
            AppEvent::PrintComplete => {
                self.printing = false;
                self.print_progress = None;
                self.push_log("Print complete".into());
            }
            AppEvent::Error(e) => {
                self.last_error = Some(e.clone());
                self.push_log(format!("Error: {}", e));
                self.printing = false;
                self.scanning = false;
            }
        }
        ctx.request_repaint();
    }

    fn render_bluetooth_section(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Bluetooth tools").strong());

        ui.horizontal(|ui| {
            if !self.connected {
                let btn_text = if self.scanning { "Scanning..." } else { "Scan & Connect" };
                if ui.add_enabled(!self.scanning, egui::Button::new(btn_text).min_size(egui::vec2(120.0, 40.0))).clicked() {
                    self.cmd_tx.try_send(BleCommand::ScanAndConnect).ok();
                    self.scanning = true;
                    self.last_error = None;
                }
            } else {
                if ui.add(egui::Button::new("Disconnect").min_size(egui::vec2(120.0, 40.0))).clicked() {
                    self.cmd_tx.try_send(BleCommand::Disconnect).ok();
                }
            }
        });

        // Status indicator
        let (status_text, color) = if self.scanning {
            ("\u{27F3} Scanning...", egui::Color32::from_rgb(0, 102, 204))
        } else if self.connected {
            ("\u{25CF} Connected", egui::Color32::from_rgb(0, 170, 0))
        } else {
            ("\u{25CF} Disconnected", egui::Color32::from_rgb(204, 0, 0))
        };
        ui.colored_label(color, status_text);

        // Battery indicator
        if let Some(pct) = self.battery_pct {
            let color = if pct > 50 {
                egui::Color32::from_rgb(0, 170, 0)
            } else if pct > 20 {
                egui::Color32::from_rgb(204, 119, 0)
            } else {
                egui::Color32::from_rgb(204, 0, 0)
            };
            ui.colored_label(color, format!("Battery: {}%", pct));
        }

        // Error display
        if let Some(ref err) = self.last_error.clone() {
            ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
        }
    }

    fn render_text_section(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Text tools").strong());

        // Justification radio buttons (display only — rendering handles it via word-wrap)
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.justification, Justification::Left, "left");
            ui.radio_value(&mut self.justification, Justification::Center, "center");
            ui.radio_value(&mut self.justification, Justification::Right, "right");
        });

        // Text input area
        egui::ScrollArea::vertical()
            .max_height(100.0)
            .id_salt("text_input")
            .show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut self.text_input)
                    .desired_width(f32::INFINITY)
                    .desired_rows(5));
            });

        // Load text file button
        if ui.add(egui::Button::new("Select a text file").min_size(egui::vec2(f32::INFINITY, 30.0))).clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Text files", &["txt"])
                .add_filter("All files", &["*"])
                .pick_file()
            {
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        self.text_input = content;
                    }
                    Err(e) => {
                        self.last_error = Some(format!("Failed to read file: {}", e));
                    }
                }
            }
        }

        // Print text button
        let can_print_text = self.connected && !self.text_input.trim().is_empty() && !self.printing;
        if ui.add_enabled(can_print_text, egui::Button::new("Print your text!").min_size(egui::vec2(f32::INFINITY, 40.0))).clicked() {
            let text = self.text_input.clone();
            self.cmd_tx.try_send(BleCommand::PrintText(text)).ok();
            self.printing = true;
            self.last_error = None;
        }
    }

    fn render_image_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label(egui::RichText::new("Image tools").strong());

        // Image preview canvas
        if let Some(ref texture) = self.image_preview_texture {
            let available = ui.available_width();
            let size = egui::vec2(available.min(300.0), 100.0);
            ui.add(egui::Image::new(texture).fit_to_exact_size(size));
        } else {
            // Placeholder white box
            let (rect, _) = ui.allocate_exact_size(egui::vec2(300.0, 100.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 0.0, egui::Color32::WHITE);
            ui.painter().rect_stroke(rect, 0.0, egui::Stroke::new(1.0, egui::Color32::GRAY), egui::StrokeKind::Middle);
        }

        // Load image file button
        if ui.add(egui::Button::new("Select an image file").min_size(egui::vec2(f32::INFINITY, 30.0))).clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
                .add_filter("All files", &["*"])
                .pick_file()
            {
                match image::open(&path) {
                    Ok(img) => {
                        // Build thumbnail texture for preview
                        let thumb = img.thumbnail(300, 100);
                        let rgb = thumb.to_rgb8();
                        let (w, h) = rgb.dimensions();
                        let pixels: Vec<egui::Color32> = rgb.pixels()
                            .map(|p| egui::Color32::from_rgb(p[0], p[1], p[2]))
                            .collect();
                        let color_image = egui::ColorImage {
                            size: [w as usize, h as usize],
                            pixels,
                            source_size: egui::vec2(w as f32, h as f32),
                        };
                        self.image_preview_texture = Some(
                            ctx.load_texture("image_preview", color_image, egui::TextureOptions::default())
                        );
                        self.current_image = Some(img);
                    }
                    Err(e) => {
                        self.last_error = Some(format!("Failed to open image: {}", e));
                    }
                }
            }
        }

        // Print image button
        let can_print_image = self.connected && self.current_image.is_some() && !self.printing;
        if ui.add_enabled(can_print_image, egui::Button::new("Print your image!").min_size(egui::vec2(f32::INFINITY, 40.0))).clicked() {
            if let Some(ref img) = self.current_image {
                self.cmd_tx.try_send(BleCommand::PrintImage(img.clone())).ok();
                self.printing = true;
                self.last_error = None;
            }
        }

        // Print progress
        if let Some((sent, total)) = self.print_progress {
            ui.label(format!("Sending... {}/{} bytes", sent, total));
            let progress = sent as f32 / total as f32;
            ui.add(egui::ProgressBar::new(progress));
        }
    }

    fn render_log_section(&self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Activity log").strong());

        let log_bg = egui::Color32::from_rgb(30, 30, 30);
        let log_fg = egui::Color32::from_rgb(212, 212, 212);

        egui::Frame::new()
            .fill(log_bg)
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .stick_to_bottom(true)
                    .id_salt("activity_log")
                    .show(ui, |ui| {
                        for entry in &self.log_entries {
                            ui.colored_label(log_fg, egui::RichText::new(entry).monospace());
                        }
                    });
            });
    }
}

impl eframe::App for PrinterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain all pending events from BLE thread
        let events: Vec<AppEvent> = {
            let mut collected = Vec::new();
            while let Ok(event) = self.evt_rx.try_recv() {
                collected.push(event);
            }
            collected
        };
        for event in events {
            self.handle_event(event, ctx);
        }

        // Keep repainting while active operations are in progress
        if self.scanning || self.printing {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.render_bluetooth_section(ui);
                ui.separator();
                self.render_text_section(ui);
                ui.separator();
                // Need to pass ctx into render_image_section for texture creation
                // We do this by temporarily cloning ctx
                let ctx_clone = ctx.clone();
                self.render_image_section(ui, &ctx_clone);
                ui.separator();
                self.render_log_section(ui);
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Send disconnect command on window close
        self.cmd_tx.try_send(BleCommand::Disconnect).ok();
        // Give the BLE thread a moment to disconnect
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}
