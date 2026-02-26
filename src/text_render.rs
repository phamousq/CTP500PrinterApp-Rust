use ab_glyph::{Font, PxScale, ScaleFont};
use image::{DynamicImage, Rgb, RgbImage};
use imageproc::drawing::draw_text_mut;
use crate::types::PRINTER_WIDTH;

const FONT_PATH: &str = "/System/Library/Fonts/Menlo.ttc";
const FONT_SIZE: f32 = 28.0;
const CANVAS_HEIGHT: u32 = 5000;

/// Render text to a bitmap image at PRINTER_WIDTH, trimmed of trailing whitespace.
/// Port of Python's `create_text` + `get_wrapped_text` + `trimImage`.
pub fn render_text_to_image(text: &str) -> Result<DynamicImage, String> {
    let font_data = std::fs::read(FONT_PATH)
        .map_err(|e| format!("Failed to read font {}: {}", FONT_PATH, e))?;

    // FontRef requires a static lifetime; use FontVec instead for owned data
    let font = ab_glyph::FontVec::try_from_vec(font_data)
        .map_err(|e| format!("Failed to parse font: {}", e))?;

    let scale = PxScale::from(FONT_SIZE);

    // Word-wrap each line of input text
    let mut wrapped_lines: Vec<String> = Vec::new();
    for line in text.lines() {
        let wrapped = get_wrapped_text(line, &font, scale, PRINTER_WIDTH as f32);
        wrapped_lines.push(wrapped);
    }
    let full_text = wrapped_lines.join("\n");

    // Create white canvas
    let mut img = RgbImage::from_pixel(PRINTER_WIDTH, CANVAS_HEIGHT, Rgb([255u8, 255, 255]));

    // Draw text line by line to track Y position
    let scaled = font.as_scaled(scale);
    let line_height = (scaled.ascent() - scaled.descent() + scaled.line_gap()).ceil() as i32;

    let mut y = 0i32;
    for line in full_text.lines() {
        draw_text_mut(&mut img, Rgb([0u8, 0, 0]), 0, y, scale, &font, line);
        y += line_height;
        if y >= CANVAS_HEIGHT as i32 {
            break;
        }
    }

    let img = DynamicImage::ImageRgb8(img);
    Ok(trim_image(img))
}

/// Word-wrap text to fit within `max_width` pixels.
/// Port of Python's `get_wrapped_text`.
fn get_wrapped_text<F: Font>(text: &str, font: &F, scale: PxScale, max_width: f32) -> String {
    let mut lines: Vec<String> = vec![String::new()];

    for word in text.split_whitespace() {
        let candidate = if lines.last().unwrap().is_empty() {
            word.to_string()
        } else {
            format!("{} {}", lines.last().unwrap(), word)
        };

        if measure_text_width(font, scale, &candidate) <= max_width {
            *lines.last_mut().unwrap() = candidate;
        } else {
            lines.push(word.to_string());
        }
    }

    // Handle empty input
    if lines.is_empty() {
        return String::new();
    }

    lines.join("\n")
}

/// Measure the pixel width of a string using glyph advance widths.
fn measure_text_width<F: Font>(font: &F, scale: PxScale, text: &str) -> f32 {
    let scaled = font.as_scaled(scale);
    let mut width = 0.0f32;
    let mut prev_glyph_id = None;

    for c in text.chars() {
        let glyph_id = scaled.glyph_id(c);
        if let Some(prev) = prev_glyph_id {
            width += scaled.kern(prev, glyph_id);
        }
        width += scaled.h_advance(glyph_id);
        prev_glyph_id = Some(glyph_id);
    }
    width
}

/// Trim trailing whitespace rows from the bottom of an image, keeping 10px padding.
/// Port of Python's `trimImage`.
fn trim_image(img: DynamicImage) -> DynamicImage {
    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();

    // Find the last non-white row from the bottom
    let mut last_content_row = 0u32;
    for y in 0..height {
        for x in 0..width {
            let p = rgb.get_pixel(x, y);
            if p[0] < 255 || p[1] < 255 || p[2] < 255 {
                last_content_row = y;
                break;
            }
        }
    }

    // Crop with 10px bottom padding, but don't exceed image height
    let crop_height = (last_content_row + 10 + 1).min(height);
    DynamicImage::ImageRgb8(rgb).crop_imm(0, 0, width, crop_height)
}
