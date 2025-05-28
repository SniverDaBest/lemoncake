use crate::FRAMEBUFFER;
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
                let pixel_index = row * width + col;
                if pixel_index < bitmap.len() {
                    self.put_pixel(x + col, y + row, bitmap[pixel_index]);
                }
            }
        }
    }

    pub fn draw_smiley(&mut self, x: usize, y: usize, color: (u8, u8, u8)) {
        // left eye
        self.put_pixel(x + 1, y, color);
        self.put_pixel(x + 1, y + 1, color);
        // right eye
        self.put_pixel(x + 5, y, color);
        self.put_pixel(x + 5, y + 1, color);
        // mouth
        self.put_pixel(x + 0, y + 4, color);
        self.put_pixel(x + 1, y + 5, color);
        self.put_pixel(x + 2, y + 5, color);
        self.put_pixel(x + 3, y + 5, color);
        self.put_pixel(x + 4, y + 5, color);
        self.put_pixel(x + 5, y + 5, color);
        self.put_pixel(x + 6, y + 4, color);
    }

    pub fn draw_sad_face(&mut self, x: usize, y: usize, color: (u8, u8, u8)) {
        // left eye
        self.put_pixel(x + 1, y, color);
        self.put_pixel(x + 1, y + 1, color);
        // right eye
        self.put_pixel(x + 5, y, color);
        self.put_pixel(x + 5, y + 1, color);
        // mouth
        self.put_pixel(x + 0, y + 5, color);
        self.put_pixel(x + 1, y + 4, color);
        self.put_pixel(x + 2, y + 4, color);
        self.put_pixel(x + 3, y + 4, color);
        self.put_pixel(x + 4, y + 4, color);
        self.put_pixel(x + 5, y + 4, color);
        self.put_pixel(x + 6, y + 5, color);
    }

    pub fn resolution(&self) -> (usize, usize) {
        return (self.fb.info().width, self.fb.info().height);
    }
}

static mut TTY_BUFFER: [char; 80 * 25] = [' '; 80 * 25];

pub struct TTY {
    width: usize,
    height: usize,
    text_buf: &'static mut [char],
    cursor_x: usize,
    cursor_y: usize,
    fg_color: (u8, u8, u8),
}

impl TTY {
    pub fn new() -> Self {
        let (mut width, mut height) = (0, 0);
        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            (width, height) = fb.resolution();
        }
        let width = width / 8;
        let height = height / 8;

        let buffer = unsafe { &mut TTY_BUFFER[..] };

        return Self {
            width,
            height,
            text_buf: buffer,
            cursor_x: 0,
            cursor_y: 0,
            fg_color: (255, 255, 255),
        };
    }

    pub fn set_char(&mut self, x: usize, y: usize, c: char, color: (u8, u8, u8)) {
        if x >= self.width || y >= self.height {
            return;
        }
        crate::font::draw_char(x * 8, y * 8, c, color);
    }

    pub fn fill_tty(&mut self, c: char) {
        for i in 0..self.text_buf.len() {
            self.text_buf[i] = c;
        }
    }

    pub fn write_str(&mut self, s: &str) {
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                if chars.peek() == Some(&'[') {
                    chars.next();
                    let mut num_buf = [0u8; 3];
                    let mut num_len = 0;
                    while let Some(&d) = chars.peek() {
                        if d >= '0' && d <= '9' && num_len < 3 {
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
                                30 => (0, 0, 0),       // Black
                                31 => (243, 139, 168), // Red
                                32 => (166, 227, 161), // Green
                                33 => (249, 226, 175), // Yellow
                                34 => (137, 180, 250), // Blue
                                35 => (203, 166, 247), // Magenta
                                36 => (0, 170, 170),   // Cyan
                                37 => (255, 255, 255), // White
                                0 => (205, 214, 244),  // Reset
                                _ => self.fg_color,
                            };
                        } else {
                            self.fg_color = (205, 214, 244);
                        }
                        continue;
                    }
                }
            }
            match c {
                '\n' => {
                    self.cursor_x = 0;
                    self.cursor_y += 1;
                }
                '\r' => {
                    self.cursor_x = 0;
                }
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
                self.cursor_y = 0;
            }
        }
    }

    pub fn get_cur_loc(&mut self) -> (usize, usize) {
        return (self.cursor_x * 8, self.cursor_y * 8);
    }

    pub fn yay(&mut self, color: Option<(u8, u8, u8)>) {
        let (x, y) = self.get_cur_loc();
        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            fb.draw_smiley(x, y, color.unwrap_or(self.fg_color));
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

    pub fn sad(&mut self, color: Option<(u8, u8, u8)>) {
        let (x, y) = self.get_cur_loc();
        if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
            fb.draw_sad_face(x, y, color.unwrap_or(self.fg_color));
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
