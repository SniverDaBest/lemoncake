use bootloader_api::info::{FrameBuffer, PixelFormat};

pub struct Framebuffer { pub fb: FrameBuffer }

impl Framebuffer {
    pub fn new(fb: FrameBuffer) -> Self {
        Framebuffer {
            fb,
        }
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, color: (u8, u8, u8)) {
        if x >= self.fb.info().width || y >= self.fb.info().height {
            return;
        }

        let pixel_index = y * self.fb.info().stride + x;
        let byte_offset = pixel_index * self.fb.info().bytes_per_pixel;

        match self.fb.info().pixel_format {
            PixelFormat::Rgb => {
                self.fb.buffer_mut()[byte_offset] = color.0;
                self.fb.buffer_mut()[byte_offset + 1] = color.1;
                self.fb.buffer_mut()[byte_offset + 2] = color.2;
            }
            PixelFormat::Bgr => {
                self.fb.buffer_mut()[byte_offset] = color.2;
                self.fb.buffer_mut()[byte_offset + 1] = color.1;
                self.fb.buffer_mut()[byte_offset + 2] = color.0;
            }
            PixelFormat::U8 => {
                self.fb.buffer_mut()[byte_offset] = color.0;
            }
            _ => {}
        }
    }

    pub fn clear_screen(&mut self, color: (u8, u8, u8)) {
        for y in 0..self.fb.info().height {
            for x in 0..self.fb.info().width {
                self.put_pixel(x, y, color);
            }
        }
    }

    pub fn draw_bitmap(
        &mut self,
        bitmap: &[(u8, u8, u8)],
        width: usize,
        height: usize,
        x: usize,
        y: usize,
    ) {
        if x + width > self.fb.info().width || y + height > self.fb.info().height {
            return;
        }

        for row in 0..height {
            for col in 0..width {
                self.put_pixel(x + col, y + row, bitmap[row * width + col]);
            }
        }
    }
}
