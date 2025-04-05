use crate::fonts;
use alloc::{format, string::String, vec, vec::*};
use fontdue::Font;
use log::{info, warn};
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};

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
        if self.check_bounds(x, y) {
            return None;
        }
        self.pixels.get_mut(y * self.width + x)
    }

    /// Returns INVALID_PARAMETER if accessing pixel that is out of bounds.
    pub fn place_pxl(&mut self, x: usize, y: usize) -> uefi::Result {
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
    pub fn draw_bitmap(
        &mut self,
        bitmap: &[BltPixel],
        bmp_width: usize,
        bmp_height: usize,
        pos_x: usize,
        pos_y: usize,
    ) -> uefi::Result {
        if bitmap.len() != bmp_width * bmp_height
            || self.check_bounds(pos_x, pos_y)
            || self.check_bounds(bmp_width, bmp_height)
        {
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
    pub fn fill_buffer(&mut self, color: BltPixel) -> uefi::Result {
        for (i, _) in self.pixels.clone().iter().enumerate() {
            self.pixels[i] = color;
        }

        self.blit()
    }

    pub fn blit(&mut self) -> uefi::Result {
        self.gop.blt(BltOp::BufferToVideo {
            buffer: &self.pixels,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (self.width, self.height),
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TTYColors {
    pub foreground: (u8, u8, u8),
    pub background: (u8, u8, u8),
}

impl TTYColors {
    pub fn new(foreground: (u8, u8, u8), background: (u8, u8, u8)) -> Self {
        return Self {
            foreground,
            background,
        };
    }

    /// Default white on black.
    pub fn default() -> Self {
        return Self {
            foreground: (255, 255, 255),
            background: (0, 0, 0),
        };
    }

    /// Black on White
    pub fn bonw() -> Self {
        return Self {
            foreground: (0, 0, 0),
            background: (255, 255, 255),
        };
    }

    /// White on Yellow
    pub fn wony() -> Self {
        return Self {
            foreground: (255, 255, 255),
            background: (255, 255, 0),
        };
    }
}

pub struct TTY {
    text_buffer: [[char; 200]; 150],
    cur_line: u8,
    cur_char: u8,
    colors: Option<TTYColors>,
}

impl TTY {
    pub const fn new() -> Self {
        return Self {
            text_buffer: [[' '; 200]; 150],
            cur_line: 0,
            cur_char: 0,
            colors: None,
        };
    }

    pub fn set_colors(&mut self, tty_colors: TTYColors) {
        self.colors = Some(tty_colors);
    }

    fn shift_buf(&mut self) {
        self.text_buffer[0] = [' '; 200];
        for i in 1..150 {
            self.text_buffer[i - 1] = self.text_buffer[i];
        }
    }

    pub fn new_line(&mut self) {
        self.cur_char = 0;
        if self.cur_line == 150 {
            self.shift_buf();
        } else {
            self.cur_line += 1;
        }
    }

    #[allow(unused_must_use)]
    pub fn clear_buf(&mut self, buf: &mut Buffer) {
        buf.fill_buffer(if self.colors.is_some() {
            BltPixel::new(
                self.colors.unwrap().background.0,
                self.colors.unwrap().background.1,
                self.colors.unwrap().background.2,
            )
        } else {
            BltPixel::new(
                TTYColors::default().background.0,
                TTYColors::default().background.1,
                TTYColors::default().background.2,
            )
        });
        self.cur_line = 0;
        self.text_buffer = [[' '; 200]; 150];
    }

    fn add_char(&mut self, c: char) -> Result<(), String> {
        if self.cur_char == 200 {
            return Err(format!(
                "Line length is 200, so character '{}' cannot be placed!",
                c
            ));
        }

        self.cur_char += 1;
        self.text_buffer[self.cur_line as usize][self.cur_char as usize] = c;

        return Ok(());
    }

    fn draw(&mut self, buf: &mut Buffer, font: &Font) {
        self.clear_buf(buf);
        for line in self.text_buffer {
            if line == [' '; 200] {
                continue
            }

            let string = line.into_iter().collect::<String>();
            fonts::draw_string(
                font.clone(),
                string.as_str(),
                12.0,
                buf,
                0,
                12 * self.cur_line as usize,
                if self.colors.is_some() {
                    self.colors.unwrap().foreground
                } else {
                    TTYColors::default().foreground
                },
            );
        }
    }

    pub fn write_str(&mut self, buf: &mut Buffer, font: &Font, s: &str) -> Result<(), String> {
        for c in s.chars() {
            if c == '\n' {
                self.new_line();
            } else if c == '\t' {
                self.write_str(buf, font, "    ")
                    .expect("Unable to write tab!");
            }

            info!("Adding Character: {}", c);
            self.add_char(c)?;
        }

        self.draw(buf, font);
        return Ok(());
    }

    pub fn set_char(&mut self, pos: (usize, usize), c: char) {
        self.text_buffer[pos.0][pos.1] = c;
    }
}
