use alloc::string::*;

#[derive(Clone, Copy)]
pub enum FontType {
    TrueType = 0,
    OpenType = 1,
    AAT = 2,
}

pub struct Font {
    filename: String,
    /// The name of the font.\
    /// Could be one of the following\
    /// JetBrains Mono
    /// Roboto
    /// Quicksand
    /// etc. (there are thousands, and I can't name ALL of them.)
    font_name: String,
    /// The size of the font in pt.
    size: u32,
    font_type: FontType
}

impl Font {
    pub fn new(filename: String, font_name: String, size: u32, font_type: FontType) -> Self {
        Self {
            filename,
            font_name,
            size,
            font_type
        }
    }

    pub fn filename(&self) -> &str {
        *&self.filename.as_str()
    }

    pub fn font_name(&self) -> &str {
        *&self.font_name.as_str()
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn font_type(&self) -> FontType {
        self.font_type
    }
}