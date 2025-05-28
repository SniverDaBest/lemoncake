//! SFS - Simple Filesystem (created by Brendan Trotter)
use super::{FSError, FSResult, Filesystem};
use crate::{ahci::AhciDevice, error, success, warning};
use alloc::{format, string::*, vec, vec::*};

#[derive(Debug)]
pub struct SuperBlock {
    last_alt_time: u64,
    /// Size is in *blocks*, not bytes.
    data_area_sz: u64,
    /// Size is in *bytes*, not blocks.
    idx_area_sz: u64,
    magic: [u8; 3],
    ver: u8,
    block_count: u64,
    /// Size is in *blocks*, not bytes.
    /// NOTE: It also includes the superblock size.
    rsv_area_sz: u32,
    block_sz: u8,
    checksum: u8,
}

impl SuperBlock {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 42 {
            error!("(SFS) Not enough data!");
            return None;
        }

        let last_alt_time = u64::from_le_bytes(data[0x194..0x19B].try_into().ok()?);
        let data_area_sz = u64::from_le_bytes(data[0x19C..0x1A3].try_into().ok()?);
        let idx_area_sz = u64::from_le_bytes(data[0x1A4..0x1AB].try_into().ok()?);
        let magic = data[0x1AC..0x1AE].try_into().ok()?;
        let ver = data[0x1AF];
        let block_count = u64::from_le_bytes(data[0x1B0..0x1B7].try_into().ok()?);
        let rsv_area_sz = u32::from_le_bytes(data[0x1B8..0x1BB].try_into().ok()?);
        let block_sz = data[0x1BC];
        let checksum = data[0x1BD];

        return Some(SuperBlock {
            last_alt_time,
            data_area_sz,
            idx_area_sz,
            magic,
            ver,
            block_count,
            rsv_area_sz,
            block_sz,
            checksum,
        });
    }

    pub fn validate_superblock(&self) -> bool {
        if self.magic != [0x53, 0x46, 0x53] {
            error!("(SFS) Magic is incorrect!");
            return false;
        }

        if self.block_sz != 2 ^ (self.block_sz + 7) {
            error!("(SFS) Block size is incorrect!");
            return false;
        }

        // TODO: Double check the checksum

        return true;
    }

    pub fn block_sz_bytes(&self) -> usize {
        return 1 << (self.block_sz as usize + 7);
    }

    pub fn total_bytes(&self) -> usize {
        return self.block_count as usize * self.block_sz_bytes();
    }

    pub fn index_area_offset(&self) -> usize {
        return self.total_bytes() - self.idx_area_sz as usize;
    }
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct IndexEntry {
    name: [u8; 48],
    entry_type: u8,
    perms: u8,
    timestamp: u64,
    start_block: u32,
    sz_bytes: u16,
}

impl IndexEntry {
    pub fn is_valid(&self) -> bool {
        return self.entry_type != 0;
    }

    pub fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&c| c == 0).unwrap_or(48);
        return core::str::from_utf8(&self.name[..end]).unwrap_or("[INVALID]");
    }
}

pub struct IndexArea {
    entries: Vec<IndexEntry>,
}

impl IndexArea {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() & 64 != 0 {
            error!("(SFS) Data length is incorrect!");
            return None;
        }

        let entries = unsafe {
            core::slice::from_raw_parts(data.as_ptr() as *const IndexEntry, data.len() / 64)
        }
        .to_vec();

        return Some(Self { entries });
    }

    pub fn find_by_name(&self, name: &str) -> Option<&IndexEntry> {
        return self
            .entries
            .iter()
            .find(|e| e.is_valid() && e.name_str() == name);
    }
    pub fn insert_entry(&mut self, entry: &IndexEntry) -> FSResult<()> {
        self.entries.push(*entry);
        return Ok(());
    }

    pub fn is_block_free(&self, block: u64, file_block_len: u64) -> bool {
        for entry in self.entries.iter() {
            if !entry.is_valid() || entry.entry_type < 0x11 || entry.entry_type > 0x12 {
                continue;
            }

            let entry_start = entry.start_block as u64;
            let entry_len = entry.sz_bytes as u64;

            let entry_blocks = (entry_len + file_block_len - 1) / file_block_len;

            if block >= entry_start && block < entry_start + entry_blocks {
                return false;
            }
        }

        return true;
    }
}

pub struct SFS<'a> {
    device: &'a mut AhciDevice,
    superblock: &'a mut SuperBlock,
    mounted: bool,
    index_area: Option<IndexArea>,
}

impl<'a> SFS<'a> {
    pub fn new(device: &'a mut AhciDevice, superblock: &'a mut SuperBlock) -> Self {
        return Self {
            device,
            superblock,
            mounted: false,
            index_area: None,
        };
    }

    pub fn load_index_area(&mut self) -> FSResult<IndexArea> {
        let index_offset = self.superblock.index_area_offset();
        let index_size = self.superblock.idx_area_sz as usize;
        let sector_size = 512;

        let sector_start = index_offset / sector_size;
        let sector_end = (index_offset + index_size + sector_size - 1) / sector_size;
        let num_sectors = sector_end - sector_start;

        let mut buf = vec![0u8; num_sectors * sector_size];

        for i in 0..num_sectors {
            let mut sector_buf = [0u8; 512];
            if !self
                .device
                .read_sector(0, (sector_start + i) as u64, &mut sector_buf)
            {
                return Err(FSError::ReadError);
            }
            buf[i * sector_size..(i + 1) * sector_size].copy_from_slice(&sector_buf);
        }

        let offset_in_first_sector = index_offset % sector_size;
        let entries_slice = &buf[offset_in_first_sector..offset_in_first_sector + index_size];

        IndexArea::from_bytes(entries_slice).ok_or(FSError::BadFS)
    }

    pub fn find_free_data_blocks(&mut self, num_blocks: usize) -> Option<u32> {
        let data_area_start = self.superblock.rsv_area_sz as u64;
        let data_area_end = data_area_start + self.superblock.data_area_sz;

        let block_size = self.superblock.block_sz_bytes();

        let index_area = self.index_area.as_ref()?; // Safely access the index area

        for i in data_area_start..=data_area_end - num_blocks as u64 {
            let mut free = true;
            for j in 0..num_blocks {
                if !index_area.is_block_free((i + j as u64) as u64, block_size as u64) {
                    free = false;
                    break;
                }
            }

            if free {
                return Some(i as u32);
            }
        }

        return None;
    }

    pub fn write_index(&mut self) -> FSResult<()> {
        let index_area = self.index_area.as_ref().ok_or(FSError::BadFS)?;
        let index_offset = self.superblock.index_area_offset();
        let index_size = self.superblock.idx_area_sz as usize;
        let sector_size = 512;

        let sector_start = index_offset / sector_size;
        let sector_end = (index_offset + index_size + sector_size - 1) / sector_size;
        let num_sectors = sector_end - sector_start;

        let mut buf = vec![0u8; num_sectors * sector_size];

        for (i, entry) in index_area.entries.iter().enumerate() {
            if i * 64 < buf.len() {
                buf[i * 64..(i + 1) * 64].copy_from_slice(unsafe {
                    core::slice::from_raw_parts(entry as *const _ as *const u8, 64)
                });
            }
        }

        for i in 0..num_sectors {
            if !self.device.write_sector(
                0,
                (sector_start + i) as u64,
                &buf[i * sector_size..(i + 1) * sector_size],
            ) {
                return Err(FSError::WriteError);
            }
        }

        return Ok(());
    }

    pub fn parent_exists(&self, path: &str) -> bool {
        if let Some(pos) = path.rfind('/') {
            let parent = &path[..pos];
            if parent.is_empty() {
                return true;
            }
            self.index_area
                .as_ref()
                .unwrap()
                .find_by_name(parent)
                .map_or(false, |e| e.entry_type == 0x11)
        } else {
            return true;
        }
    }

    pub fn list_dir(&self, path: &str) -> Vec<&IndexEntry> {
        let prefix = if path.ends_with('/') {
            path.to_string()
        } else if path.is_empty() {
            "".to_string()
        } else {
            format!("{}/", path)
        };

        self.index_area
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .filter(|e| e.is_valid() && e.name_str().starts_with(&prefix))
            .collect()
    }
}

impl<'a> Filesystem for SFS<'a> {
    fn mount(&mut self) -> FSResult<()> {
        if self.mounted {
            error!("(SFS) FS already mounted!");
            return Err(FSError::AlreadyMounted);
        }

        if self.superblock.validate_superblock() {
            error!("(SFS) Superblock is invalid!");
            return Err(FSError::BadFS);
        }

        let index = self.load_index_area()?;
        self.index_area = Some(index);

        self.mounted = true;
        success!("(SFS) Mounted!");

        warning!("(SFS) Mounting/Unmounting doesn't really do anything... yet.");

        return Ok(());
    }

    fn unmount(&mut self) -> FSResult<()> {
        if !self.mounted {
            error!("(SFS) FS not mounted!");
            return Err(FSError::NotMounted);
        }

        warning!("(SFS) Mounting/Unmounting doesn't really do anything... yet.");

        self.mounted = false;

        return Ok(());
    }

    fn read_file(&mut self, path: &str, buf: &mut [u8]) -> Result<usize, FSError> {
        let index = self.index_area.as_ref().ok_or(FSError::BadFS)?;

        let entry = index.find_by_name(path).ok_or(FSError::NotFound)?;
        if entry.entry_type != 0x12 {
            error!("(SFS) Invalid entry type!");
            return Err(FSError::BadFile);
        }

        let block_size = self.superblock.block_sz_bytes();
        let offset = entry.start_block as usize * block_size;
        let file_size = entry.sz_bytes as usize;

        let sector_size = 512;
        let start_sector = offset / sector_size;
        let end_sector = (offset + file_size + sector_size - 1) / sector_size;

        let mut file_buf = vec![0u8; end_sector - start_sector * sector_size];
        for i in start_sector..end_sector {
            let mut sector_buf = [0u8; 512];

            if !self.device.read_sector(0, i as u64, &mut sector_buf) {
                error!("(SFS) Failed to read sector {}!", i);
                return Err(FSError::ReadError);
            }

            file_buf[(i - start_sector) * 512..(i - start_sector + 1) * 512]
                .copy_from_slice(&sector_buf);
        }

        let slice = &file_buf[offset % 512..offset % 512 + file_size];
        let to_copy = slice.len().min(buf.len());
        buf[..to_copy].copy_from_slice(&slice[..to_copy]);

        return Ok(to_copy);
    }

    fn write_file(&mut self, path: &str, buf: &[u8]) -> FSResult<()> {
        {
            let index = self.index_area.as_mut().ok_or(FSError::BadFS)?;

            if let Some(e) = index.find_by_name(path) {
                error!("(SFS) File already exists!\nFile entry: {:?}", e);
                return Err(FSError::AlreadyExists);
            }
        }

        let block_size = self.superblock.block_sz_bytes();
        let num_blocks = (buf.len() + block_size - 1) / block_size;
        let start_block = self
            .find_free_data_blocks(num_blocks)
            .ok_or(FSError::NoSpace)?;
        let index = self.index_area.as_mut().ok_or(FSError::BadFS)?;

        let start_offset = start_block as usize * block_size;
        let start_sector = start_offset / 512;

        for (i, chunk) in buf.chunks(512).enumerate() {
            let mut sector_buf = [0u8; 512];
            sector_buf[..chunk.len()].copy_from_slice(chunk);
            if !self
                .device
                .write_sector(0, (start_sector + i) as u64, &sector_buf)
            {
                error!("(SFS) Failed to write sector {}!", start_sector + i);
                return Err(FSError::WriteError);
            }
        }

        let entry = IndexEntry {
            name: {
                let mut name = [0u8; 48];
                let bytes = path.as_bytes();
                if bytes.len() >= 48 {
                    return Err(FSError::NameTooLong);
                }
                name[..bytes.len()].copy_from_slice(bytes);
                name[bytes.len()] = 0;
                name
            },
            entry_type: 0x12,
            perms: 0,
            timestamp: 0,
            start_block: start_block as u32,
            sz_bytes: buf.len() as u16, // FIXME: split into continuation entries if >64KB
        };

        index.insert_entry(&entry)?;

        self.write_index()?;

        return Ok(());
    }

    fn remove_file(&mut self, path: &str) -> FSResult<()> {
        error!("(SFS) Removing files is not implemented yet!");
        return Err(FSError::WriteError);
    }

    fn create_dir(&mut self, path: &str) -> FSResult<()> {
        let index = self.index_area.as_mut().ok_or(FSError::WriteError)?;

        if index.find_by_name(path).is_some() {
            return Err(FSError::AlreadyExists);
        }

        let entry = IndexEntry {
            name: {
                let mut name = [0u8; 48];
                let bytes = path.as_bytes();
                if bytes.len() >= 48 {
                    return Err(FSError::NameTooLong);
                }
                name[..bytes.len()].copy_from_slice(bytes);
                name[bytes.len()] = 0;
                name
            },
            entry_type: 0x11,
            perms: 0,
            timestamp: 0,
            start_block: 0,
            sz_bytes: 0,
        };

        index.insert_entry(&entry)?;
        self.write_index()?;
        return Ok(());
    }

    fn remove_dir(&mut self, path: &str) -> FSResult<()> {
        error!("(SFS) Removing directories is not implemented yet!");
        return Err(FSError::WriteError);
    }
}
