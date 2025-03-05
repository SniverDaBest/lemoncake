#[repr(C, packed)]
pub struct Header {
    /// The name of the partition SHFS is stored on.\
    /// MUST be [u8; 16]. (16 u8s)
    pub name: [u8; 16],
    /// The size of each piece in KB.\
    /// No less than 8KB.
    pub piece_sz: u32,
    /// The amount of pieces in the FS.
    pub piece_count: u16,
    /// The amount of pieces in the FS which are being used.
    pub used_piece_count: u16,
    /// The size of the reserved area
    pub reserved_area_sz: u32,
}

#[repr(C, packed)]
pub struct PieceHeader {
    /// The ID of the piece.\
    /// For example, the 16th piece will be ID 15.\
    /// This is because it's zero-indexed.
    pub piece_id: u16,
    /// The ID of the next piece.
    /// This is because sometimes, pieces will go unused,\
    /// if a big file is deleted, and there will be space inbetween.
    pub next_piece: u16,
    /// This is set to any number >0 if a file cuts through multiple pieces.\
    /// If the file cuts through the entire piece, then the header will be put into the reserved area.
    pub file_cut_sz: u32,
}

impl Header {
    pub fn new(name: [u8; 16], piece_sz: u32, piece_count: u16) -> Self {
        Self {
            name, piece_sz, piece_count, used_piece_count: 0, reserved_area_sz: 5*(piece_count+4) as u32
        }
    }
}

impl PieceHeader {
    pub fn new(piece_id: u16, next_piece: u16) -> Self {
        Self { piece_id, next_piece, file_cut_sz: 0 }
    }
}
