use std::time::Duration;
use image::DynamicImage;
use tokio::sync::mpsc::Sender;
use btleplug::api::{Peripheral as _, WriteType};
use btleplug::platform::Peripheral;
use btleplug::api::Characteristic;

use crate::escpos::image_to_escpos_bytes;
use crate::types::{AppEvent, CHUNK_SIZE};

/// Full print sequence: initialize → start → image data → end.
/// Port of Python's `PrinterConnect.print_image()`.
pub async fn print_image(
    peripheral: &Peripheral,
    write_char: &Characteristic,
    img: DynamicImage,
    evt_tx: &Sender<AppEvent>,
) {
    let buf = image_to_escpos_bytes(&img);
    let img_w = img.width();
    let img_h = img.height();

    // Initialize printer (ESC @)
    evt_tx.send(AppEvent::Log("Sent: initialize printer (ESC @)".into())).await.ok();
    if let Err(e) = write_chunked(peripheral, write_char, &[0x1b, 0x40], evt_tx).await {
        evt_tx.send(AppEvent::Error(format!("Print error: {}", e))).await.ok();
        return;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Start print sequence
    evt_tx.send(AppEvent::Log("Sent: start print sequence".into())).await.ok();
    if let Err(e) = write_chunked(peripheral, write_char, &[0x1d, 0x49, 0xf0, 0x19], evt_tx).await {
        evt_tx.send(AppEvent::Error(format!("Print error: {}", e))).await.ok();
        return;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Image data
    let log_msg = format!("Sent: image data ({} bytes, {}x{}px)", buf.len(), img_w, img_h);
    evt_tx.send(AppEvent::Log(log_msg)).await.ok();
    if let Err(e) = write_chunked(peripheral, write_char, &buf, evt_tx).await {
        evt_tx.send(AppEvent::Error(format!("Print error: {}", e))).await.ok();
        return;
    }
    let delay_ms = ((buf.len() as f64 / 5000.0) * 1000.0).max(500.0) as u64;
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;

    // End print sequence
    evt_tx.send(AppEvent::Log("Sent: end print sequence".into())).await.ok();
    if let Err(e) = write_chunked(peripheral, write_char, &[0x0a, 0x0a, 0x0a, 0x9a], evt_tx).await {
        evt_tx.send(AppEvent::Error(format!("Print error: {}", e))).await.ok();
        return;
    }
    tokio::time::sleep(Duration::from_millis(1000)).await;

    evt_tx.send(AppEvent::Log("Print complete".into())).await.ok();
    evt_tx.send(AppEvent::PrintComplete).await.ok();
}

/// Write data in CHUNK_SIZE-sized chunks using write-with-response.
/// Port of Python's `PrinterConnect._write_bytes()`.
async fn write_chunked(
    peripheral: &Peripheral,
    write_char: &Characteristic,
    data: &[u8],
    evt_tx: &Sender<AppEvent>,
) -> Result<(), btleplug::Error> {
    let total = data.len();
    let total_chunks = data.chunks(CHUNK_SIZE).count();

    for (i, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
        peripheral.write(write_char, chunk, WriteType::WithResponse).await?;

        if total_chunks > 10 && i % 10 == 0 {
            let sent = ((i + 1) * CHUNK_SIZE).min(total);
            evt_tx.send(AppEvent::PrintProgress { sent, total }).await.ok();
        }
    }
    Ok(())
}
