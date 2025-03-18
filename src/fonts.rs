use fontdue::{Font, FontSettings};
use log::info;
use uefi::proto::console::gop::BltPixel;
use crate::{display::Buffer, read_file};
use alloc::{format, vec::*, string::*};

pub fn draw_glyph(font: &Font, glyph: char, scale: f32, buf: &mut Buffer, x: usize, y: usize) {
    let raster = font.rasterize(glyph, scale);
    info!("Raster: {:#?}", raster);
    let mut final_glyph: Vec<BltPixel> = Vec::new();
    for x in raster.1 {
        final_glyph.push(BltPixel::new(x,x,x));
    }
    buf.draw_bitmap(final_glyph.as_ref(), raster.0.width, raster.0.height, x, y).expect("Unable to draw glyph bitmap!");
}

/// Does not support `\n`!
pub fn draw_string(font: Font, text: String, scale: f32, buf: &mut Buffer, x: usize, y: usize) {
    let mut x_offset = x;
    let line_metrics = font.horizontal_line_metrics(scale)
        .expect("Unable to get line metrics!");
    let baseline = y as isize + line_metrics.ascent as isize;
    for c in text.chars() {
        let metrics = font.metrics(c, scale);
        // Calculate the top y-coordinate of the glyph by subtracting ymax from the baseline
        let glyph_y = baseline - metrics.bounds.height as isize;
        draw_glyph(&font, c, scale, buf, x_offset, glyph_y as usize);
        x_offset += metrics.advance_width as usize;
    }
}