use crate::display::Buffer;
use alloc::{string::*, vec::*, sync::Arc};
use core::{fmt::{self, Write}, ptr::addr_of_mut};
use fontdue::Font;
use log::info;
use uefi::proto::console::gop::BltPixel;

/// Draws a single glyph to the screen. (DOES NOT SUPPORT \n, \t, etc. why would you even want that either?)
pub fn draw_glyph(font: &Font, glyph: char, scale: f32, buf: &mut Buffer, x: usize, y: usize) {
    let raster = font.rasterize(glyph, scale);
    info!("Raster: {:#?}", raster);
    let mut final_glyph: Vec<BltPixel> = Vec::new();
    for x in raster.1 {
        final_glyph.push(BltPixel::new(x, x, x));
    }
    buf.draw_bitmap(final_glyph.as_ref(), raster.0.width, raster.0.height, x, y)
        .expect("Unable to draw glyph bitmap!");
}

/// Draws a string to the screen.
pub fn draw_string(font: Font, text: String, scale: f32, buf: &mut Buffer, x: usize, y: usize) {
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
            x_offset += metrics.advance_width as usize * 4;
        } else {
            let baseline = y_offset as isize + line_metrics.ascent as isize;

            // Calculate the top y-coordinate of the glyph by subtracting ymax from the baseline
            let glyph_y = baseline - metrics.bounds.height as isize;
            draw_glyph(&font, c, scale, buf, x_offset, glyph_y as usize);
            x_offset += metrics.advance_width as usize;
        }
    }
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::fonts::draw_string(format_args!($($arg)*)); 
    };
}
