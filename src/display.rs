use alloc::vec;
use alloc::vec::Vec;
use uefi::{
    Result, boot,
    proto::{
        console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput},
        rng::Rng,
    },
};

#[derive(Clone, Copy)]
pub struct Point {
    x: f32,
    y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

pub struct Buffer {
    pub width: usize,
    pub height: usize,
    pixels: Vec<BltPixel>,
}

impl Buffer {
    pub fn new(width: usize, height: usize) -> Buffer {
        Self {
            width,
            height,
            pixels: vec![BltPixel::new(0, 0, 0); width * height],
        }
    }

    pub fn get_pxl(&mut self, x: usize, y: usize) -> Option<&mut BltPixel> {
        self.pixels.get_mut(y * self.width + x)
    }

    pub fn place_pxl(&self, gop: &mut GraphicsOutput, x: usize, y: usize) -> Result {
        gop.blt(BltOp::BufferToVideo {
            buffer: &self.pixels,
            src: BltRegion::SubRectangle {
                coords: (x, y),
                px_stride: self.width,
            },
            dest: (x, y),
            dims: (1, 1),
        })
    }

    pub fn blit(&mut self, gop: &mut GraphicsOutput) -> Result {
        gop.blt(BltOp::BufferToVideo {
            buffer: &self.pixels,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (self.width, self.height),
        })
    }
}

/// Get a random `usize` value.
fn get_random_usize(rng: &mut Rng) -> usize {
    let mut buf = [0; size_of::<usize>()];
    rng.get_rng(None, &mut buf).expect("get_rng failed");
    usize::from_le_bytes(buf)
}
