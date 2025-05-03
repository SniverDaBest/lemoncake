use core::intrinsics::roundf32;

use fontdue::Font;
use alloc::vec::*;
use crate::display::Framebuffer;

/// Draws a single glyph to the screen with a specified color.
#[allow(unused_must_use)]
pub fn draw_glyph(
    font: &Font,
    glyph: char,
    scale: f32,
    buf: &mut Framebuffer,
    x: usize,
    y: usize,
    color: (u8, u8, u8),
) {
    if glyph == ' ' {
        return;
    }

    let (metrics, bitmap) = font.rasterize(glyph, scale);

    if bitmap.is_empty() {
        panic!("Glyph '{}' failed to rasterize!", glyph);
    }

    let mut final_glyph: Vec<(u8, u8, u8)> = Vec::new();
    for &px in &bitmap {
        let alpha = px as f32 / 255.0;

        // Blend color with intensity
        let blended_r = (color.0 as f32 * alpha) as u8;
        let blended_g = (color.1 as f32 * alpha) as u8;
        let blended_b = (color.2 as f32 * alpha) as u8;

        final_glyph.push((blended_r, blended_g, blended_b));
    }

    buf.draw_bitmap(final_glyph.as_ref(), metrics.width, metrics.height, x, y);
}

/// Draws a string to the screen with a specified color.
pub fn draw_string(
    font: Font,
    text: &str,
    scale: f32,
    buf: &mut Framebuffer,
    x: usize,
    y: usize,
    color: (u8, u8, u8),
) {
    let mut x_offset = x;
    let mut y_offset = y;
    let line_metrics = font
        .horizontal_line_metrics(scale)
        .expect("Unable to get line metrics!");
    for c in text.chars() {
        let metrics = font.metrics(c, scale);

        if c == '\n' {
            x_offset = x;
            y_offset += 14;
        } else if c == '\t' {
            x_offset += unsafe { roundf32(metrics.advance_width) } as usize * 4;
        } else {
            let glyph_y = y_offset as isize + (line_metrics.ascent - metrics.bounds.ymin) as isize;

            draw_glyph(&font, c, scale, buf, x_offset, glyph_y as usize, color);
            x_offset += metrics.advance_width as usize;
        }
    }
}