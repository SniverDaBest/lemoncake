use alloc::vec;
use alloc::vec::Vec;
use uefi::{
    Result,
    proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput},
};
use log::warn;

pub struct Buffer<'a> {
    pub width: usize,
    pub height: usize,
    pixels: Vec<BltPixel>,
    pub gop: &'a mut GraphicsOutput,
}

impl<'a> Buffer<'a> {
    pub fn new(gop: &'a mut GraphicsOutput, width: usize, height: usize) -> Buffer<'a> {
        Self {
            width,
            height,
            pixels: vec![BltPixel::new(0, 0, 0); width * height],
            gop,
        }
    }

    fn check_bounds(&self, x: usize, y: usize) -> bool {
        if x > self.width || y > self.height {
            warn!("Attempted to access out-of-bounds area. Failing...");
            return true;
        }
        return false;
    }

    /// Will return None if accessing pixel that is out of bounds.
    pub fn get_pxl(&mut self, x: usize, y: usize) -> Option<&mut BltPixel> {
        if self.check_bounds(x, y) { return None }
        self.pixels.get_mut(y * self.width + x)
    }

    /// Returns INVALID_PARAMETER if accessing pixel that is out of bounds.
    pub fn place_pxl(&mut self, x: usize, y: usize) -> Result {
        if self.check_bounds(x, y) {
            return Err(uefi::Status::INVALID_PARAMETER.into());
        }
        self.gop.blt(BltOp::BufferToVideo {
            buffer: &self.pixels,
            src: BltRegion::SubRectangle {
                coords: (x, y),
                px_stride: self.width,
            },
            dest: (x, y),
            dims: (1, 1),
        })
    }

    /// Returns INVALID_PARAMETER if accessing out-of-bounds area of framebuffer.
    pub fn draw_bitmap(&mut self, bitmap: &[BltPixel], bmp_width: usize, bmp_height: usize, pos_x: usize, pos_y: usize) -> Result {  
        if bitmap.len() != bmp_width * bmp_height || self.check_bounds(pos_x, pos_y) || self.check_bounds(bmp_width, bmp_height) {
            return Err(uefi::Status::INVALID_PARAMETER.into());
        }

        for y in 0..bmp_height {
            if pos_x + y >= self.height {
                break;
            }
            for x in 0..bmp_width {
                if pos_x + x >= self.width {
                    break;
                }
                let bmp_idx = y * bmp_width + x;
                let buf_idx = (pos_y + y) * self.width + (pos_x + x);
                self.pixels[buf_idx] = bitmap[bmp_idx];
            }
        }

        self.blit()
    }

    /// Fills the entire buffer with one color.
    pub fn fill_buffer(&mut self, color: BltPixel) -> Result {
        for (i, _) in self.pixels.clone().iter().enumerate() {
            self.pixels[i] = color;
        }

        self.blit()
    }

    pub fn blit(&mut self) -> Result {
        self.gop.blt(BltOp::BufferToVideo {
            buffer: &self.pixels,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (self.width, self.height),
        })
    }
}
