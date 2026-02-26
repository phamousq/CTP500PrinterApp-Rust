mod app;
mod ble;
mod escpos;
mod printer;
mod text_render;
mod types;

use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<types::BleCommand>(32);
    let (evt_tx, evt_rx) = tokio::sync::mpsc::channel::<types::AppEvent>(256);

    // Spawn a dedicated OS thread to own the Tokio runtime for BLE operations.
    // This mirrors the Python app's background asyncio event loop on a daemon thread.
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        rt.block_on(ble::ble_task(cmd_rx, evt_tx));
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CTP500 Printer Control")
            .with_inner_size([520.0, 820.0])
            .with_min_inner_size([520.0, 820.0]),
        ..Default::default()
    };

    eframe::run_native(
        "CTP500 Printer Control",
        options,
        Box::new(|_cc| Ok(Box::new(app::PrinterApp::new(cmd_tx, evt_rx)))),
    )
}
