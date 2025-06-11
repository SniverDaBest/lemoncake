use crate::FRAMEBUFFER;
use alloc::vec::Vec;
use zune_png::{
    PngDecoder,
    zune_core::{colorspace::ColorSpace, result::DecodingResult},
};

pub fn decode(data: &[u8]) -> DecodingResult {
    return PngDecoder::new(data)
        .decode()
        .expect("Unable to decode PNG data!");
}

pub fn draw_png(data: &[u8], x: usize, y: usize) {
    let mut decoder = PngDecoder::new(data);
    let pxls = decoder.decode().expect("Unable to decode PNG data!");

    if decoder.get_colorspace().expect("Unable to get colorspace!") != ColorSpace::RGBA {
        panic!("Only PNGS with the RGBA colorspace are supported.");
    }

    let (width, height) = decoder
        .get_dimensions()
        .expect("Unable to get image dimensions!");

    let rgb_tuples: Vec<(u8, u8, u8, u8)> = pxls
        .u8()
        .expect("Unable to get pixels!")
        .chunks_exact(4)
        .map(|chunk| (chunk[0], chunk[1], chunk[2], chunk[3]))
        .collect();

    if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
        fb.draw_bitmap(&rgb_tuples, width, height, x, y);
    }
}
