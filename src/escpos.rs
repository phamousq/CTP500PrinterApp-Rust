use image::{DynamicImage, GrayImage, ImageBuffer, Luma, imageops};
use crate::types::PRINTER_WIDTH;

/// Convert a DynamicImage to the ESC/POS raster byte sequence for the CTP500.
/// This is a direct port of Python's `_image_to_bytes(im)`.
pub fn image_to_escpos_bytes(img: &DynamicImage) -> Vec<u8> {
    // 1. Scale down if wider than printer width
    let img = if img.width() > PRINTER_WIDTH {
        let new_height = (img.height() as f64 * PRINTER_WIDTH as f64 / img.width() as f64) as u32;
        img.resize(PRINTER_WIDTH, new_height, imageops::FilterType::Lanczos3)
    } else {
        img.clone()
    };

    // 2. Pad to printer width if narrower
    let img = if img.width() < PRINTER_WIDTH {
        let mut padded = DynamicImage::new_rgb8(PRINTER_WIDTH, img.height());
        // Fill with white
        for y in 0..img.height() {
            for x in 0..PRINTER_WIDTH {
                padded.as_mut_rgb8().unwrap().put_pixel(x, y, image::Rgb([255, 255, 255]));
            }
        }
        imageops::overlay(&mut padded, &img, 0, 0);
        padded
    } else {
        img
    };

    // 3. Convert to grayscale and threshold to 1-bit logical
    //    pixel >= 128 → white (255), < 128 → black (0)
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();

    // 4. Pad width to multiple of 8
    let padded_width = (w + 7) & !7;

    // Build a padded grayscale image (white fill for padding)
    let mut padded_gray: GrayImage = ImageBuffer::from_pixel(padded_width, h, Luma([255u8]));
    for y in 0..h {
        for x in 0..w {
            let p = gray.get_pixel(x, y)[0];
            padded_gray.put_pixel(x, y, Luma([p]));
        }
    }

    // 5. Invert: white (255) → 0, black (0) → 255 (matching PIL ImageOps.invert)
    // 6. Pack pixels MSB-first into bytes
    let bytes_per_row = (padded_width / 8) as usize;
    let mut pixel_data: Vec<u8> = Vec::with_capacity(bytes_per_row * h as usize);

    for y in 0..h {
        for byte_idx in 0..bytes_per_row {
            let mut byte = 0u8;
            for bit in 0..8u32 {
                let x = byte_idx as u32 * 8 + bit;
                let pixel = padded_gray.get_pixel(x, y)[0];
                // Invert: dark pixels (< 128) become 1, light pixels become 0
                let ink = if pixel < 128 { 1u8 } else { 0u8 };
                byte |= ink << (7 - bit);
            }
            pixel_data.push(byte);
        }
    }

    // 7. Assemble ESC/POS GS v 0 raster command
    // Header: GS v 0 <mode> <xL> <xH> <yL> <yH> <data>
    let width_bytes = bytes_per_row as u16;
    let height_lines = h as u16;

    let mut out = Vec::with_capacity(4 + 4 + pixel_data.len());
    out.extend_from_slice(&[0x1d, 0x76, 0x30, 0x00]); // GS v 0 mode=0
    out.extend_from_slice(&width_bytes.to_le_bytes());  // xL, xH
    out.extend_from_slice(&height_lines.to_le_bytes()); // yL, yH
    out.extend_from_slice(&pixel_data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escpos_header() {
        // Create a simple 10x10 white image
        let img = DynamicImage::new_rgb8(10, 10);
        let bytes = image_to_escpos_bytes(&img);

        // Header should be GS v 0 0x00
        assert_eq!(&bytes[0..4], &[0x1d, 0x76, 0x30, 0x00]);

        // Width should be padded to 384, so bytes_per_row = 384/8 = 48
        let width_bytes = u16::from_le_bytes([bytes[4], bytes[5]]);
        assert_eq!(width_bytes, 48);

        // Height should be 10
        let height = u16::from_le_bytes([bytes[6], bytes[7]]);
        assert_eq!(height, 10);

        // Total size: 8 header + 48 * 10 pixels
        assert_eq!(bytes.len(), 8 + 48 * 10);
    }

    #[test]
    fn test_escpos_wide_image_scaled() {
        // Image wider than 384 should be scaled down
        let img = DynamicImage::new_rgb8(800, 400);
        let bytes = image_to_escpos_bytes(&img);

        let width_bytes = u16::from_le_bytes([bytes[4], bytes[5]]);
        assert_eq!(width_bytes, 48); // 384/8

        // Height should be proportionally scaled: 400 * (384/800) = 192
        let height = u16::from_le_bytes([bytes[6], bytes[7]]);
        assert_eq!(height, 192);
    }

    #[test]
    fn test_escpos_inversion() {
        // A fully black image should produce all-0xFF bytes (after inversion: black→1)
        let img = DynamicImage::ImageRgb8(image::ImageBuffer::from_pixel(
            384, 1, image::Rgb([0u8, 0, 0]),
        ));
        let bytes = image_to_escpos_bytes(&img);
        // All pixel bytes should be 0xFF (all ink)
        let pixel_bytes = &bytes[8..];
        assert!(pixel_bytes.iter().all(|&b| b == 0xFF));
    }
}
