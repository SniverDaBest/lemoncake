use crate::{FRAMEBUFFER, font::FONT_HEIGHT};
use bootloader_api::info::{FrameBuffer, PixelFormat};
use core::fmt::{self, Write};

pub struct Framebuffer {
    pub fb: FrameBuffer,
}

impl Framebuffer {
    pub fn new(fb: FrameBuffer) -> Self {
        Framebuffer { fb }
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
        let width = self.fb.info().width;
        let height = self.fb.info().height;

        match self.fb.info().pixel_format {
            PixelFormat::Rgb => {
                for y in 0..height {
                    for x in 0..width {
                        let pixel_index = y * self.fb.info().stride + x;
                        let byte_offset = pixel_index * self.fb.info().bytes_per_pixel;

                        self.fb.buffer_mut()[byte_offset] = color.0;
                        self.fb.buffer_mut()[byte_offset + 1] = color.1;
                        self.fb.buffer_mut()[byte_offset + 2] = color.2;
                    }
                }
            }
            PixelFormat::Bgr => {
                for y in 0..height {
                    for x in 0..width {
                        let pixel_index = y * self.fb.info().stride + x;
                        let byte_offset = pixel_index * self.fb.info().bytes_per_pixel;

                        self.fb.buffer_mut()[byte_offset] = color.2;
                        self.fb.buffer_mut()[byte_offset + 1] = color.1;
                        self.fb.buffer_mut()[byte_offset + 2] = color.0;
                    }
                }
            }
            PixelFormat::U8 => {
                for y in 0..height {
                    for x in 0..width {
                        let pixel_index = y * self.fb.info().stride + x;
                        let byte_offset = pixel_index * self.fb.info().bytes_per_pixel;

                        self.fb.buffer_mut()[byte_offset] = (color.0 + color.1 + color.2) / 3;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn draw_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: (u8, u8, u8)) {
        if x >= self.fb.info().width || y >= self.fb.info().height || x + w <= x || y + h <= y {
            return;
        }

        let x = x.min(self.fb.info().width - w);
        let y = y.min(self.fb.info().height - h);

        match self.fb.info().pixel_format {
            PixelFormat::Rgb => {
                for line_y in y..(y + h) {
                    let start_idx = (line_y * self.fb.info().stride + x) * 3;

                    for i in 0..w {
                        self.fb.buffer_mut()[start_idx + i * 3] = color.0;
                        self.fb.buffer_mut()[start_idx + i * 3 + 1] = color.1;
                        self.fb.buffer_mut()[start_idx + i * 3 + 2] = color.2;
                    }
                }
            }
            PixelFormat::Bgr => {
                for line_y in y..(y + h) {
                    let start_idx = (line_y * self.fb.info().stride + x) * 3;

                    for i in 0..w {
                        self.fb.buffer_mut()[start_idx + i * 3] = color.2;
                        self.fb.buffer_mut()[start_idx + i * 3 + 1] = color.1;
                        self.fb.buffer_mut()[start_idx + i * 3 + 2] = color.0;
                    }
                }
            }
            PixelFormat::U8 => {
                let gray = (color.0 + color.1 + color.2) / 3;
                for line_y in y..(y + h) {
                    let start_idx = line_y * self.fb.info().stride + x;

                    for i in 0..w {
                        self.fb.buffer_mut()[start_idx + i] = gray;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn draw_bitmap(
        &mut self,
        bitmap: &[(u8, u8, u8, u8)],
        width: usize,
        height: usize,
        x: usize,
        y: usize,
    ) {
        let info = self.fb.info();
        let w = info.width;
        let h = info.height;

        let mw = w.saturating_sub(x).min(width);
        let mh = h.saturating_sub(y).min(height);

        if mw == 0 || mh == 0 {
            return;
        }

        let row_sd = width;

        for row in 0..mh {
            let base_index = row * row_sd;
            for col in 0..mw {
                let pixel_index = base_index + col;
                if let Some(&(r, g, b, a)) = bitmap.get(pixel_index) && a != 0{
                    self.put_pixel(x + col, y + row, (r, g, b));
                }
            }
        }
    }

    pub fn draw_smiley(&mut self, x: usize, y: usize, color: (u8, u8, u8, u8)) {
        let (r, g, b, a) = color;
        if a == 0 {
            return;
        }
        // left eye
        self.put_pixel(x + 1, y, (r, g, b));
        self.put_pixel(x + 1, y + 1, (r, g, b));
        // right eye
        self.put_pixel(x + 5, y, (r, g, b));
        self.put_pixel(x + 5, y + 1, (r, g, b));
        // mouth
        self.put_pixel(x, y + 4, (r, g, b));
        self.put_pixel(x + 1, y + 5, (r, g, b));
        self.put_pixel(x + 2, y + 5, (r, g, b));
        self.put_pixel(x + 3, y + 5, (r, g, b));
        self.put_pixel(x + 4, y + 5, (r, g, b));
        self.put_pixel(x + 5, y + 5, (r, g, b));
        self.put_pixel(x + 6, y + 4, (r, g, b));
    }

    pub fn draw_sad_face(&mut self, x: usize, y: usize, color: (u8, u8, u8, u8)) {
        let (r, g, b, a) = color;
        if a == 0 {
            return;
        }
        // left eye
        self.put_pixel(x + 1, y, (r, g, b));
        self.put_pixel(x + 1, y + 1, (r, g, b));
        // right eye
        self.put_pixel(x + 5, y, (r, g, b));
        self.put_pixel(x + 5, y + 1, (r, g, b));
        // mouth
        self.put_pixel(x, y + 5, (r, g, b));
        self.put_pixel(x + 1, y + 4, (r, g, b));
        self.put_pixel(x + 2, y + 4, (r, g, b));
        self.put_pixel(x + 3, y + 4, (r, g, b));
        self.put_pixel(x + 4, y + 4, (r, g, b));
        self.put_pixel(x + 5, y + 4, (r, g, b));
        self.put_pixel(x + 6, y + 5, (r, g, b));
    }

    pub fn resolution(&self) -> (usize, usize) {
        return (self.fb.info().width, self.fb.info().height);
    }
}

#[derive(Clone, Copy)]
pub struct Cell {
    ch: char,
    color: (u8, u8, u8, u8),
}

impl Cell {
    pub fn empty(color: (u8, u8, u8, u8)) -> Self {
        return Self { ch: '\x00', color };
    }
}

static mut TTY_BUFFER: [Cell; 130 * 50] = [Cell {
    ch: '\x00',
    color: (255, 255, 255, 255),
}; 130 * 50];

#[cfg(feature = "catppuccin-colorscheme")]
pub const BACKGROUND: (u8, u8, u8) = (30, 30, 46);
#[cfg(feature = "catppuccin-colorscheme")]
pub const WHITE: (u8, u8, u8, u8) = (205, 214, 244, 255);
#[cfg(feature = "catppuccin-colorscheme")]
pub const BLACK: (u8, u8, u8, u8) = (17, 17, 27, 255);
#[cfg(feature = "catppuccin-colorscheme")]
pub const RED: (u8, u8, u8, u8) = (243, 139, 168, 255);
#[cfg(feature = "catppuccin-colorscheme")]
pub const GREEN: (u8, u8, u8, u8) = (166, 227, 161, 255);
#[cfg(feature = "catppuccin-colorscheme")]
pub const YELLOW: (u8, u8, u8, u8) = (249, 226, 175, 255);
#[cfg(feature = "catppuccin-colorscheme")]
pub const BLUE: (u8, u8, u8, u8) = (137, 180, 250, 255);
#[cfg(feature = "catppuccin-colorscheme")]
pub const MAGENTA: (u8, u8, u8, u8) = (203, 166, 247, 255);
#[cfg(feature = "catppuccin-colorscheme")]
pub const CYAN: (u8, u8, u8, u8) = (0, 170, 170, 255);

#[cfg(not(feature = "catppuccin-colorscheme"))]
pub const BACKGROUND: (u8, u8, u8) = (0, 0, 0);
#[cfg(not(feature = "catppuccin-colorscheme"))]
pub const WHITE: (u8, u8, u8, u8) = (255, 255, 255, 255);
#[cfg(not(feature = "catppuccin-colorscheme"))]
pub const BLACK: (u8, u8, u8, u8) = (0, 0, 0, 255);
#[cfg(not(feature = "catppuccin-colorscheme"))]
pub const RED: (u8, u8, u8, u8) = (255, 0, 0, 255);
#[cfg(not(feature = "catppuccin-colorscheme"))]
pub const GREEN: (u8, u8, u8, u8) = (0, 255, 0, 255);
#[cfg(not(feature = "catppuccin-colorscheme"))]
pub const YELLOW: (u8, u8, u8, u8) = (255, 255, 0, 255);
#[cfg(not(feature = "catppuccin-colorscheme"))]
pub const BLUE: (u8, u8, u8, u8) = (0, 0, 255, 255);
#[cfg(not(feature = "catppuccin-colorscheme"))]
pub const MAGENTA: (u8, u8, u8, u8) = (255, 0, 255, 255);
#[cfg(not(feature = "catppuccin-colorscheme"))]
pub const CYAN: (u8, u8, u8, u8) = (0, 255, 255, 255);

pub struct TTY {
    width: usize,
    height: usize,
    text_buf: &'static mut [Cell],
    cursor_x: usize,
    cursor_y: usize,
    fg_color: (u8, u8, u8, u8),
}

impl Default for TTY {
    fn default() -> Self {
        return Self::new();
    }
}

impl TTY {
    pub fn new() -> Self {
        let (mut width, mut height) = (0, 0);
        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            (width, height) = fb.resolution();
        }
        let width = (width / 8).min(130);
        let height = (height / 8).min(50);

        let buffer = unsafe { &mut TTY_BUFFER[..] };

        return Self {
            width,
            height,
            text_buf: buffer,
            cursor_x: 0,
            cursor_y: 0,
            fg_color: (255, 255, 255, 255),
        };
    }

    pub fn set_char(&mut self, x: usize, y: usize, c: char, color: (u8, u8, u8, u8)) {
        if x >= self.width || y >= self.height {
            return;
        }

        self.text_buf[y * self.width + x] = Cell { ch: c, color };
        crate::font::draw_char_psf(x * 8, y * unsafe { FONT_HEIGHT }, c, color);
    }

    pub fn get_char(&self, x: usize, y: usize) -> Option<char> {
        if x >= self.width || y >= self.height {
            return None;
        }
        return Some(self.text_buf[y * self.width + x].ch);
    }

    pub fn clear_tty(&mut self) {
        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            fb.clear_screen(BACKGROUND);
        }

        for i in 0..self.text_buf.len() {
            self.text_buf[i] = Cell::empty(self.fg_color);
        }

        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    #[cfg(not(feature = "clear-on-scroll"))]
    pub fn scroll_up(&mut self) {
        for y in 1..self.height {
            for x in 0..self.width {
                let from = self.text_buf[y * self.width + x];
                self.text_buf[(y - 1) * self.width + x] = from;
            }
        }

        for x in 0..self.width {
            self.text_buf[(self.height - 1) * self.width + x] = Cell {
                ch: '\x00',
                color: self.fg_color,
            };
        }

        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            fb.clear_screen(BACKGROUND);
        }

        for y in 0..self.height {
            for x in 0..self.width {
                let cell = self.text_buf[y * self.width + x];
                if cell.ch != '\x00' {
                    crate::font::draw_char_psf(
                        x * 8,
                        y * unsafe { FONT_HEIGHT },
                        cell.ch,
                        cell.color,
                    );
                }
            }
        }
    }

    pub fn delete(&mut self, bksp: bool) {
        if self.cursor_y >= self.height {
            return;
        }

        let row_start = self.cursor_y * self.width;
        let row_end = row_start + self.width;

        if bksp {
            if self.cursor_x == 0 {
                return;
            }
            self.cursor_x -= 1;

            for x in self.cursor_x..(self.width - 1) {
                self.text_buf[row_start + x] = self.text_buf[row_start + x + 1];
            }

            self.text_buf[row_end - 1] = Cell {
                ch: '\x00',
                color: self.fg_color,
            };
        } else {
            if self.cursor_x >= self.width {
                return;
            }

            for x in self.cursor_x..(self.width - 1) {
                self.text_buf[row_start + x] = self.text_buf[row_start + x + 1];
            }

            self.text_buf[row_end - 1] = Cell {
                ch: '\x00',
                color: self.fg_color,
            };
        }

        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            fb.clear_screen(BACKGROUND);
        }

        for x in 0..self.width {
            let cell = self.text_buf[row_start + x];
            crate::font::draw_char_psf(
                x * 8,
                self.cursor_y * unsafe { FONT_HEIGHT },
                cell.ch,
                cell.color,
            );
        }
    }

    pub fn write_str(&mut self, s: &str) {
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' && chars.peek() == Some(&'[') {
                chars.next();
                let mut num_buf = [0u8; 3];
                let mut num_len = 0;
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() && num_len < 3 {
                        num_buf[num_len] = d as u8;
                        num_len += 1;
                        chars.next();
                    } else {
                        break;
                    }
                }
                if chars.peek() == Some(&'m') {
                    chars.next();
                    if num_len > 0 {
                        let code = core::str::from_utf8(&num_buf[..num_len]).unwrap_or("0");
                        let code = code.parse::<u8>().unwrap_or(0);
                        self.fg_color = match code {
                            30 => BLACK,    // Black
                            31 => RED, // Red
                            32 => GREEN, // Green
                            33 => YELLOW, // Yellow
                            34 => BLUE, // Blue
                            35 => MAGENTA, // Magenta
                            36 => CYAN,   // Cyan
                            0 | 37 => WHITE,  // White/Reset
                            _ => self.fg_color,
                        };
                    } else {
                        self.fg_color = WHITE;
                    }
                    continue;
                }
            }
            match c {
                '\n' => {
                    self.cursor_x = 0;
                    self.cursor_y += 1;
                }
                '\r' => self.cursor_x = 0,
                '\x08' => self.delete(true),
                '\x7f' => self.delete(false),
                _ => {
                    self.set_char(self.cursor_x, self.cursor_y, c, self.fg_color);
                    self.cursor_x += 1;
                }
            }
            if self.cursor_x >= self.width {
                self.cursor_x = 0;
                self.cursor_y += 1;
            }
            if self.cursor_y >= self.height {
                #[cfg(not(feature = "clear-on-scroll"))]
                {
                    self.scroll_up();
                    self.cursor_y = self.height - 1;
                }

                #[cfg(feature = "clear-on-scroll")]
                self.clear_tty();
            }
        }
    }

    pub fn get_cur_loc(&mut self) -> (usize, usize) {
        return (self.cursor_x * 8, self.cursor_y * unsafe { FONT_HEIGHT });
    }

    pub fn yay(&mut self, color: Option<(u8, u8, u8, u8)>) {
        let (x, y) = self.get_cur_loc();
        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            fb.draw_smiley(x, y + unsafe { FONT_HEIGHT } / 4, color.unwrap_or(self.fg_color));
            self.cursor_x += 1;
            if self.cursor_x >= self.width {
                self.cursor_x = 0;
                self.cursor_y += 1;
            }
            if self.cursor_y >= self.height {
                self.cursor_y = 0;
            }
        }
    }

    pub fn sad(&mut self, color: Option<(u8, u8, u8, u8)>) {
        let (x, y) = self.get_cur_loc();
        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            fb.draw_sad_face(x, y + unsafe { FONT_HEIGHT } / 4, color.unwrap_or(self.fg_color));
            self.cursor_x += 1;
            if self.cursor_x >= self.width {
                self.cursor_x = 0;
                self.cursor_y += 1;
            }
            if self.cursor_y >= self.height {
                self.cursor_y = 0;
            }
        }
    }
}

impl Write for TTY {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_str(s);
        return Ok(());
    }
}
