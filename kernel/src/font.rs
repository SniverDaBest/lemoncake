static FONT_DATA: &[u8] = include_bytes!("../../assets/font.psf");
pub static mut FONT_HEIGHT: usize = 8;

const PSF1_MAGIC: u16 = 0x0436;
const PSF2_MAGIC: u32 = 0x864ab572;

#[repr(C, packed)]
struct PSF1Header {
    magic: u16,
    mode: u8,
    char_size: u8,
}

#[repr(C, packed)]
struct PSF2Header {
    magic: u32,
    version: u32,
    header_size: u32,
    flags: u32,
    num_glyphs: u32,
    bytes_per_glyph: u32,
    height: u32,
    width: u32,
}

pub struct Font<'a> {
    glyphs: &'a [u8],
    num_glyphs: usize,
    bytes_per_glyph: usize,
    height: usize,
    width: usize,
}

pub fn parse_psf(font: &'static [u8]) -> Option<Font<'static>> {
    if font.len() >= size_of::<PSF2Header>() {
        let hdr_ptr = font.as_ptr() as *const PSF2Header;
        let hdr = unsafe { &*hdr_ptr };
        if hdr.magic == PSF2_MAGIC {
            let hdr_sz = hdr.header_size as usize;
            if font.len() < hdr_sz {
                return None;
            }
            let glyphs = &font[hdr_sz..];
            let bytes_per_glyph = hdr.bytes_per_glyph as usize;
            let num_glyphs = hdr.num_glyphs as usize;
            if glyphs.len() < bytes_per_glyph.checked_mul(num_glyphs)? {
                return None;
            }

            unsafe {
                FONT_HEIGHT = hdr.height as usize;
            }

            return Some(Font {
                glyphs,
                num_glyphs,
                bytes_per_glyph,
                height: hdr.height as usize,
                width: hdr.width as usize,
            });
        }
    }

    if font.len() >= size_of::<PSF1Header>() {
        let hdr_ptr = font.as_ptr() as *const PSF1Header;
        let hdr = unsafe { &*hdr_ptr };
        if hdr.magic == PSF1_MAGIC {
            let header_sz = size_of::<PSF1Header>();
            let glyphs = &font[header_sz..];
            let bytes_per_glyph = hdr.char_size as usize;
            let num_glyphs = if (hdr.mode & 0x01) != 0 { 512 } else { 256 };
            if glyphs.len() < bytes_per_glyph.checked_mul(num_glyphs)? {
                return None;
            }

            unsafe {
                FONT_HEIGHT = bytes_per_glyph;
            }

            return Some(Font {
                glyphs,
                num_glyphs,
                bytes_per_glyph,
                height: bytes_per_glyph,
                width: 8,
            });
        }
    }

    None
}

pub fn draw_char_psf(x: usize, y: usize, ch: char, color: (u8, u8, u8, u8)) {
    let font = match parse_psf(FONT_DATA) {
        Some(f) => f,
        None => return,
    };

    let code = ch as u32;
    let mut glyph_index = if code <= 0xFF { code as usize } else { 0usize };

    if glyph_index >= font.num_glyphs {
        if code >= 32 && (code as usize - 32) < font.num_glyphs {
            glyph_index = code as usize - 32;
        } else {
            glyph_index = 0;
        }
    }

    let bpg = font.bytes_per_glyph;
    let start = glyph_index.checked_mul(bpg).unwrap_or(usize::MAX);
    if start == usize::MAX {
        return;
    }
    let end = start + bpg;
    if end > font.glyphs.len() {
        return;
    }
    let glyph = &font.glyphs[start..end];

    let msb_left = true;

    for row in 0..font.height {
        let bits_in_row = font.width;
        let row_byte_offset = (row * ((bits_in_row + 7) / 8)) as usize;
        for bit in 0..bits_in_row {
            let byte_idx = row_byte_offset + (bit / 8) as usize;
            if byte_idx >= glyph.len() {
                break;
            }
            let b = glyph[byte_idx];
            let bit_in_byte = bit % 8;
            let pixel_on = if msb_left {
                ((b >> (7 - bit_in_byte)) & 1) != 0
            } else {
                ((b >> bit_in_byte) & 1) != 0
            };
            if pixel_on {
                if let Some(fb) = crate::FRAMEBUFFER.lock().as_mut() {
                    fb.put_pixel(
                        x + bit as usize,
                        y + row as usize,
                        (color.0, color.1, color.2),
                    );
                }
            }
        }
    }
}
