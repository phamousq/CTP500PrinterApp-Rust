mod app;
mod ble;
mod escpos;
mod printer;
mod text_render;
mod types;

use std::sync::Arc;
use dioxus::prelude::*;
use dioxus_desktop::{Config, WindowBuilder};
use tokio::sync::Mutex;

use app::{App, AppState};

fn main() {
    env_logger::init();

    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<types::BleCommand>(32);
    let (evt_tx, evt_rx) = tokio::sync::mpsc::channel::<types::AppEvent>(256);

    // Spawn a dedicated OS thread owning the Tokio runtime for BLE operations.
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        rt.block_on(ble::ble_task(cmd_rx, evt_tx));
    });

    // Wrap channels in Arc<Mutex> so they can be shared into the Dioxus context.
    let state = Arc::new(Mutex::new(AppState { cmd_tx, evt_rx }));

    let window = WindowBuilder::new()
        .with_title("CTP500 Printer Control")
        .with_inner_size(dioxus_desktop::tao::dpi::LogicalSize::new(520.0, 820.0))
        .with_min_inner_size(dioxus_desktop::tao::dpi::LogicalSize::new(520.0, 820.0));

    LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window))
        .with_context(state)
        .launch(App);
}
