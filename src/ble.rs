use std::time::Duration;
use futures::StreamExt;
use tokio::sync::mpsc::{Receiver, Sender};
use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Manager, Peripheral};
use btleplug::api::Characteristic;

use crate::types::{
    AppEvent, BleCommand, WRITE_CHAR_UUID, NOTIFY_CHAR_UUID,
    printer_name_regex, parse_battery, CHUNK_SIZE,
};
use crate::printer::print_image;
use crate::text_render::render_text_to_image;

struct BleState {
    peripheral: Peripheral,
    write_char: Characteristic,
}

/// Main BLE task that runs on a dedicated Tokio runtime.
/// Loops on cmd_rx, dispatching BLE operations, sending events back via evt_tx.
pub async fn ble_task(mut cmd_rx: Receiver<BleCommand>, evt_tx: Sender<AppEvent>) {
    let mut state: Option<BleState> = None;

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            BleCommand::ScanAndConnect => {
                evt_tx.send(AppEvent::ScanStarted).await.ok();
                evt_tx.send(AppEvent::Log("Scanning for compatible printers (10s)...".into())).await.ok();
                match scan_and_connect(&evt_tx).await {
                    Ok(Some(new_state)) => {
                        state = Some(new_state);
                    }
                    Ok(None) => {
                        evt_tx.send(AppEvent::Log("No compatible printer found nearby".into())).await.ok();
                        evt_tx.send(AppEvent::Disconnected).await.ok();
                    }
                    Err(e) => {
                        evt_tx.send(AppEvent::Log(format!("Scan error: {}", e))).await.ok();
                        evt_tx.send(AppEvent::Disconnected).await.ok();
                    }
                }
            }

            BleCommand::Disconnect => {
                if let Some(ref s) = state {
                    disconnect_peripheral(&s.peripheral, &evt_tx).await;
                }
                state = None;
                evt_tx.send(AppEvent::Disconnected).await.ok();
            }

            BleCommand::PrintImage(img) => {
                if let Some(ref s) = state {
                    print_image(&s.peripheral, &s.write_char, img, &evt_tx).await;
                } else {
                    evt_tx.send(AppEvent::Log("Print aborted: not connected".into())).await.ok();
                }
            }

            BleCommand::PrintText { text, font_path, font_size } => {
                match render_text_to_image(&text, &font_path, font_size) {
                    Ok(img) => {
                        if let Some(ref s) = state {
                            print_image(&s.peripheral, &s.write_char, img, &evt_tx).await;
                        } else {
                            evt_tx.send(AppEvent::Log("Print aborted: not connected".into())).await.ok();
                        }
                    }
                    Err(e) => {
                        evt_tx.send(AppEvent::Error(format!("Text render error: {}", e))).await.ok();
                    }
                }
            }
        }
    }
}

/// Scan for a compatible printer and connect to the first found.
/// Port of Python's `PrinterConnect._scan_and_connect()`.
async fn scan_and_connect(evt_tx: &Sender<AppEvent>) -> Result<Option<BleState>, Box<dyn std::error::Error>> {
    let manager = Manager::new().await?;
    // Let CoreBluetooth initialize before scanning
    tokio::time::sleep(Duration::from_millis(200)).await;

    let adapters = manager.adapters().await?;
    let adapter = adapters.into_iter().next().ok_or("No Bluetooth adapter found")?;

    adapter.start_scan(ScanFilter::default()).await?;

    let mut event_stream = adapter.events().await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);

    let mut found_peripheral: Option<Peripheral> = None;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        match tokio::time::timeout(remaining, event_stream.next()).await {
            Ok(Some(btleplug::api::CentralEvent::DeviceDiscovered(id))) => {
                let peripheral = adapter.peripheral(&id).await?;
                if let Ok(Some(props)) = peripheral.properties().await {
                    if let Some(name) = &props.local_name {
                        if printer_name_regex().is_match(name) {
                            evt_tx.send(AppEvent::Log(format!("Found: {}", name))).await.ok();
                            found_peripheral = Some(peripheral);
                            break;
                        }
                    }
                }
            }
            Ok(Some(_)) => {} // Ignore other events
            Ok(None) | Err(_) => break, // Stream ended or timeout
        }
    }

    adapter.stop_scan().await.ok();

    let peripheral = match found_peripheral {
        Some(p) => p,
        None => return Ok(None),
    };

    // Connect
    let address = if let Ok(Some(props)) = peripheral.properties().await {
        props.address.to_string()
    } else {
        "unknown".to_string()
    };
    evt_tx.send(AppEvent::Log(format!("Connecting to {}...", address))).await.ok();

    peripheral.connect().await?;
    peripheral.discover_services().await?;

    let characteristics = peripheral.characteristics();

    let write_char = characteristics.iter()
        .find(|c| c.uuid.to_string().eq_ignore_ascii_case(WRITE_CHAR_UUID))
        .ok_or("Write characteristic not found")?
        .clone();

    let notify_char = characteristics.iter()
        .find(|c| c.uuid.to_string().eq_ignore_ascii_case(NOTIFY_CHAR_UUID))
        .ok_or("Notify characteristic not found")?
        .clone();

    // Subscribe to notifications
    peripheral.subscribe(&notify_char).await?;

    evt_tx.send(AppEvent::Log(format!("Connected (chunk size: {} bytes)", CHUNK_SIZE))).await.ok();
    evt_tx.send(AppEvent::Connected).await.ok();

    // Request printer status (battery etc.) — same as Python's \x1e\x47\x03
    peripheral.write(&write_char, &[0x1e, 0x47, 0x03], WriteType::WithResponse).await.ok();

    // Spawn a task to drain notifications
    let evt_tx_clone = evt_tx.clone();
    let peripheral_clone = peripheral.clone();
    tokio::spawn(async move {
        if let Ok(mut stream) = peripheral_clone.notifications().await {
            while let Some(data) = stream.next().await {
                let text = String::from_utf8_lossy(&data.value)
                    .trim()
                    .trim_end_matches(',')
                    .to_string();
                evt_tx_clone.send(AppEvent::Log(format!("Printer status: {}", text))).await.ok();

                if let Some(pct) = parse_battery(&data.value) {
                    evt_tx_clone.send(AppEvent::BatteryLevel(pct)).await.ok();
                }
            }
        }
    });

    Ok(Some(BleState { peripheral, write_char }))
}

/// Disconnect from the peripheral cleanly.
/// Port of Python's `PrinterConnect._disconnect()`.
async fn disconnect_peripheral(peripheral: &Peripheral, evt_tx: &Sender<AppEvent>) {
    evt_tx.send(AppEvent::Log("Disconnecting...".into())).await.ok();
    if let Err(e) = peripheral.disconnect().await {
        evt_tx.send(AppEvent::Log(format!("Disconnect error: {}", e))).await.ok();
    } else {
        evt_tx.send(AppEvent::Log("Disconnected".into())).await.ok();
    }
}
