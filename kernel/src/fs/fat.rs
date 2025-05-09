#![allow(dead_code)]

use super::{FSError, Filesystem};
use crate::{ahci::AhciDevice, error, info};
use alloc::{format, string::*, vec, vec::*};

#[derive(Debug)]
struct DirEntry {
    name: [u8; 11],
    attr: u8,
    cluster_high: u16,
    cluster_low: u16,
    size: u32,
}

impl DirEntry {
    fn is_directory(&self) -> bool {
        return self.attr & 0x10 != 0;
    }

    fn full_cluster(&self) -> u32 {
        return ((self.cluster_high as u32) << 16) | self.cluster_low as u32;
    }
}

pub fn split_path(path: &str) -> Vec<&str> {
    return path
        .trim_start_matches('/')
        .split('/')
        .filter(|p| !p.is_empty())
        .collect();
}

pub struct FAT<'a> {
    device: &'a mut AhciDevice,
    mounted: bool,
    current_cluster: u32,

    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    num_fats: u8,
    sectors_per_fat: u32,
    root_cluster: u32,
    fat_start_sector: u64,
}

impl<'a> FAT<'a> {
    pub fn validate(&mut self) -> Result<(), FSError> {
        let mut buf = [0u8; 512];
        if self.device.read_sector(0, 0, &mut buf) == false {
            error!("Unable to read sector 0!");
            return Err(FSError::ReadError);
        };
        if &buf[0x36..0x3E] != b"FAT32   " {
            error!("Not a FAT32 filesystem!");
            return Err(FSError::BadFS);
        }

        return Ok(());
    }

    fn cluster_to_lba(&self, cluster: u32) -> u64 {
        let first_data_sector =
            self.reserved_sectors as u64 + (self.num_fats as u64 * self.sectors_per_fat as u64);
        let cluster_number = cluster - 2;
        return first_data_sector + (cluster_number as u64 * self.sectors_per_cluster as u64);
    }

    fn read_cluster(&mut self, cluster: u32, buf: &mut [u8]) -> Result<(), FSError> {
        let lba = self.cluster_to_lba(cluster);
        for i in 0..self.sectors_per_cluster as u64 {
            let offset = i as usize * self.bytes_per_sector as usize;
            let slice = &mut buf[offset..offset + self.bytes_per_sector as usize];
            if !self.device.read_sector(0, lba + i, slice) {
                return Err(FSError::ReadError);
            }
        }
        return Ok(());
    }

    fn fat_entry_lba(&self, cluster: u32) -> u64 {
        let fat_offset = cluster as u64 * 4;
        let fat_sector = fat_offset / self.bytes_per_sector as u64;
        return self.reserved_sectors as u64 + fat_sector;
    }

    fn next_cluster(&mut self, cluster: u32) -> Result<u32, FSError> {
        let lba = self.fat_entry_lba(cluster);
        let mut sector = vec![0u8; self.bytes_per_sector as usize];
        if !self.device.read_sector(0, lba, &mut sector) {
            return Err(FSError::ReadError);
        }

        let fat_offset = (cluster as usize * 4) % self.bytes_per_sector as usize;
        let entry = u32::from_le_bytes([
            sector[fat_offset],
            sector[fat_offset + 1],
            sector[fat_offset + 2],
            sector[fat_offset + 3],
        ]) & 0x0FFF_FFFF;

        if entry >= 0x0FFFFFF8 {
            return Ok(0);
        }

        return Ok(entry);
    }

    fn write_cluster(&mut self, cluster: u32, buf: &[u8]) -> Result<(), FSError> {
        let lba = self.cluster_to_lba(cluster);
        for i in 0..self.sectors_per_cluster as u64 {
            let offset = i as usize * self.bytes_per_sector as usize;
            let slice = &buf[offset..offset + self.bytes_per_sector as usize];
            if !self.device.write_sector(0, lba + i, slice) {
                return Err(FSError::WriteError);
            }
        }
        return Ok(());
    }

    fn utf16_chunks(name: &str) -> Vec<[u16; 13]> {
        let utf16: Vec<u16> = name.encode_utf16().collect();
        let mut chunks = vec![];
        let mut i = 0;

        while i < utf16.len() {
            let mut chunk = [0xFFFF; 13];
            for j in 0..13 {
                if i + j < utf16.len() {
                    chunk[j] = utf16[i + j];
                } else if i + j == utf16.len() {
                    chunk[j] = 0x0000;
                }
            }
            chunks.push(chunk);
            i += 13;
        }

        return chunks;
    }

    fn parse_8_3_name(raw: &[u8]) -> String {
        let name = String::from_utf8_lossy(&raw[0..8]).trim().to_string();
        let ext = String::from_utf8_lossy(&raw[8..11]).trim().to_string();
        if !ext.is_empty() {
            return format!("{}.{}", name, ext);
        } else {
            return name;
        }
    }

    fn generate_8_3(name: &str) -> [u8; 11] {
        let mut name_part = [b' '; 8];
        let mut ext_part = [b' '; 3];

        let parts: Vec<&str> = name.split('.').collect();
        if !parts.is_empty() {
            for (i, b) in parts[0].bytes().take(8).enumerate() {
                name_part[i] = b;
            }
        }
        if parts.len() > 1 {
            for (i, b) in parts[1].bytes().take(3).enumerate() {
                ext_part[i] = b;
            }
        }

        let mut full = [0u8; 11];
        full[..8].copy_from_slice(&name_part);
        full[8..].copy_from_slice(&ext_part);
        return full;
    }

    fn lfn_checksum(short_name: &[u8; 11]) -> u8 {
        let mut sum = 0u8;
        for &b in short_name.iter() {
            sum = (((sum & 1) << 7) | (sum >> 1)).wrapping_add(b);
        }
        return sum;
    }

    fn find_in_directory(&mut self, start_cluster: u32, name: &str) -> Result<DirEntry, FSError> {
        let mut cluster = start_cluster;
        let target_name = name.to_ascii_lowercase();

        while cluster < 0x0FFFFFF8 {
            let mut buf =
                vec![0u8; self.bytes_per_sector as usize * self.sectors_per_cluster as usize];
            self.read_cluster(cluster, &mut buf)?;

            let mut i = 0;
            while i < buf.len() {
                let entry = &buf[i..i + 32];

                if entry[0] == 0x00 {
                    return Err(FSError::BadFS);
                }

                if entry[0] == 0xE5 {
                    i += 32;
                    continue;
                }

                if entry[11] == 0x0F {
                    let mut lfn_raw = Vec::new();

                    let mut j = i;
                    let mut lfn_entries = Vec::new();

                    while j < buf.len() {
                        let e = &buf[j..j + 32];
                        if e[11] != 0x0F {
                            break;
                        }
                        lfn_entries.push(e.to_vec());
                        j += 32;
                    }

                    lfn_entries.reverse();

                    for e in lfn_entries.iter() {
                        for pair in e[1..11]
                            .chunks(2)
                            .chain(e[14..26].chunks(2))
                            .chain(e[28..32].chunks(2))
                        {
                            let ch = u16::from_le_bytes([pair[0], pair[1]]);
                            if ch == 0x0000 || ch == 0xFFFF {
                                continue;
                            }
                            lfn_raw.push(ch);
                        }
                    }

                    let lfn_str = String::from_utf16_lossy(&lfn_raw).to_ascii_lowercase();

                    if j < buf.len() {
                        let std_entry = &buf[j..j + 32];
                        let size = u32::from_le_bytes([
                            std_entry[28],
                            std_entry[29],
                            std_entry[30],
                            std_entry[31],
                        ]);

                        if lfn_str == target_name {
                            return Ok(DirEntry {
                                name: std_entry[0..11].try_into().unwrap_or([0; 11]),
                                attr: std_entry[11],
                                cluster_high: u16::from_le_bytes([std_entry[20], std_entry[21]]),
                                cluster_low: u16::from_le_bytes([std_entry[26], std_entry[27]]),
                                size,
                            });
                        }
                    }

                    i = j + 32;
                } else {
                    let raw_name = &entry[0..11];
                    let formatted = Self::parse_8_3_name(raw_name).to_ascii_lowercase();
                    if formatted == target_name {
                        let size = u32::from_le_bytes([entry[28], entry[29], entry[30], entry[31]]);
                        return Ok(DirEntry {
                            name: raw_name.try_into().unwrap_or([0; 11]),
                            attr: entry[11],
                            cluster_high: u16::from_le_bytes([entry[20], entry[21]]),
                            cluster_low: u16::from_le_bytes([entry[26], entry[27]]),
                            size,
                        });
                    }
                    i += 32;
                }
            }

            cluster = self.next_cluster(cluster)?;
        }

        return Err(FSError::BadFS);
    }

    pub fn allocate_clusters(&mut self, count: usize) -> Result<u32, FSError> {
        let fat_sectors = self.sectors_per_fat;
        let entries_per_sector = self.bytes_per_sector / 4;
        let mut allocated = Vec::new();

        'outer: for sector in 0..fat_sectors {
            let mut buf = [0u8; 512];
            if !self.device.read_sector(
                self.fat_start_sector as usize + sector as usize,
                0,
                &mut buf,
            ) {
                return Err(FSError::ReadError);
            }

            for entry in 0..entries_per_sector {
                let i = entry * 4;
                let val = u32::from_le_bytes([
                    buf[i as usize],
                    buf[i as usize + 1],
                    buf[i as usize + 2],
                    buf[i as usize + 3],
                ]);
                if val == 0 {
                    let cluster_number = (sector * entries_per_sector as u32 + entry as u32) as u32;
                    allocated.push(cluster_number);
                    if allocated.len() == count {
                        break 'outer;
                    }
                }
            }
        }

        if allocated.len() < count {
            return Err(FSError::WriteError);
        }

        for i in 0..allocated.len() {
            let curr = allocated[i];
            let next = if i == allocated.len() - 1 {
                0x0FFFFFFF
            } else {
                allocated[i + 1]
            };

            self.set_fat_entry(curr, next)?;
        }

        return Ok(allocated[0]);
    }

    pub fn set_fat_entry(&mut self, cluster: u32, value: u32) -> Result<(), FSError> {
        let fat_offset = cluster * 4;
        let sector = self.fat_start_sector + (fat_offset / self.bytes_per_sector as u32) as u64;
        let offset = (fat_offset % self.bytes_per_sector as u32) as usize;

        let mut buf = [0u8; 512];
        if !self.device.read_sector(sector as usize, 0, &mut buf) {
            return Err(FSError::ReadError);
        }

        buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());

        if !self.device.write_sector(sector as usize, 0, &buf) {
            return Err(FSError::WriteError);
        }

        return Ok(());
    }

    pub fn write_file_data(&mut self, start_cluster: u32, data: &[u8]) -> Result<(), FSError> {
        let mut cluster = start_cluster;
        let mut offset = 0;
        let cluster_size = (self.bytes_per_sector as usize) * (self.sectors_per_cluster as usize);

        while offset < data.len() {
            let end = usize::min(offset + cluster_size, data.len());
            let chunk = &data[offset..end];
            let mut buf = [0u8; 8192];
            buf[..chunk.len()].copy_from_slice(chunk);
            self.write_cluster(cluster, &buf)?;

            offset = end;

            if offset < data.len() {
                cluster = self.next_cluster(cluster)?;
            }
        }

        return Ok(());
    }

    fn init_directory_cluster(&mut self, cluster: u32, parent_cluster: u32) -> Result<(), FSError> {
        let mut buf = [0u8; 512 * 1];
        buf.fill(0x00);

        buf[0] = b'.';
        buf[1..11].fill(b' ');
        buf[11] = 0x10;
        buf[26..28].copy_from_slice(&(cluster as u16).to_le_bytes());
        buf[20..22].copy_from_slice(&((cluster >> 16) as u16).to_le_bytes());

        buf[32] = b'.';
        buf[33] = b'.';
        buf[34..43].fill(b' ');
        buf[43] = 0x10;
        buf[58..60].copy_from_slice(&(parent_cluster as u16).to_le_bytes());
        buf[52..54].copy_from_slice(&((parent_cluster >> 16) as u16).to_le_bytes());

        self.write_cluster(cluster, &buf)?;
        return Ok(());
    }

    fn insert_dir_entry(
        &mut self,
        parent_cluster: u32,
        name: &str,
        cluster: u32,
        is_dir: bool,
    ) -> Result<(), FSError> {
        let short_name = Self::generate_8_3(name);
        let checksum = Self::lfn_checksum(&short_name);
        let chunks = Self::utf16_chunks(name);
        let entry_count = chunks.len();

        let mut dir_buf = [0u8; 4096];
        self.read_cluster(parent_cluster, &mut dir_buf)?;

        let mut i = 0;
        while i + ((entry_count + 1) * 32) <= dir_buf.len() {
            let mut empty = true;
            for j in 0..(entry_count + 1) {
                if dir_buf[i + j * 32] != 0x00 && dir_buf[i + j * 32] != 0xE5 {
                    empty = false;
                    break;
                }
            }

            if empty {
                for (n, chunk) in chunks.iter().enumerate().rev() {
                    let entry_offset = i + (chunks.len() - 1 - n) * 32;
                    let mut e = [0u8; 32];
                    let seq = (n + 1) as u8;
                    e[0] = if n == chunks.len() - 1 {
                        0x40 | seq
                    } else {
                        seq
                    };
                    e[11] = 0x0F;
                    e[13] = checksum;

                    for k in 0..5 {
                        let b = chunk[k].to_le_bytes();
                        e[1 + k * 2] = b[0];
                        e[2 + k * 2] = b[1];
                    }
                    for k in 0..6 {
                        let b = chunk[5 + k].to_le_bytes();
                        e[14 + k * 2] = b[0];
                        e[15 + k * 2] = b[1];
                    }
                    for k in 0..2 {
                        let b = chunk[11 + k].to_le_bytes();
                        e[28 + k * 2] = b[0];
                        e[29 + k * 2] = b[1];
                    }

                    dir_buf[entry_offset..entry_offset + 32].copy_from_slice(&e);
                }

                let offset = i + entry_count * 32;
                let mut s = [0u8; 32];
                s[..11].copy_from_slice(&short_name);
                s[11] = if is_dir { 0x10 } else { 0x20 };
                s[26..28].copy_from_slice(&(cluster as u16).to_le_bytes());
                s[20..22].copy_from_slice(&((cluster >> 16) as u16).to_le_bytes());
                s[28..32].copy_from_slice(&0u32.to_le_bytes());

                dir_buf[offset..offset + 32].copy_from_slice(&s);
                self.write_cluster(parent_cluster, &dir_buf)?;
                return Ok(());
            }

            i += 32;
        }

        return Err(FSError::WriteError);
    }
}

impl<'a> Filesystem for FAT<'a> {
    fn mount(&mut self) -> Result<(), FSError> {
        info!("Mounting FAT device...");
        if self.mounted {
            error!("Device already mounted!");
            return Err(FSError::AlreadyMounted);
        }

        let mut buf = [0u8; 512];
        if self.device.read_sector(0, 0, &mut buf) == false {
            error!("Unable to read sector 0!");
            return Err(FSError::ReadError);
        };
        if &buf[0x36..0x3E] != b"FAT32   " {
            error!("Not a FAT32 filesystem!");
            return Err(FSError::BadFS);
        }

        self.bytes_per_sector = u16::from_le_bytes([buf[0x0B], buf[0x0C]]);
        self.sectors_per_cluster = buf[0x0D];
        self.reserved_sectors = u16::from_le_bytes([buf[0x0E], buf[0x0F]]);
        self.num_fats = buf[0x10];
        self.sectors_per_fat = u32::from_le_bytes([buf[0x24], buf[0x25], buf[0x26], buf[0x27]]);
        self.root_cluster = u32::from_le_bytes([buf[0x2C], buf[0x2D], buf[0x2E], buf[0x2F]]);
        self.fat_start_sector = self.reserved_sectors as u64;
        self.mounted = true;
        self.mounted = true;
        return Ok(());
    }

    fn unmount(&mut self) -> Result<(), FSError> {
        info!("Unmounting FAT AHCI device...");
        if !self.mounted {
            error!("Can't unmount device that is not mounted!");
            return Err(FSError::NotMounted);
        }
        self.mounted = false;
        return Ok(());
    }

    fn read_file(&mut self, path: &str, buf: &mut [u8]) -> Result<usize, FSError> {
        if !self.mounted {
            return Err(FSError::NotMounted);
        }

        let parts = split_path(path);
        if parts.is_empty() {
            return Err(FSError::BadFS);
        }

        let (dirs, filename) = parts.split_at(parts.len() - 1);
        let mut dir_cluster = self.root_cluster;

        for dir in dirs {
            let entry = self.find_in_directory(dir_cluster, dir)?;
            if !entry.is_directory() {
                return Err(FSError::BadFS);
            }
            dir_cluster = entry.full_cluster();
        }

        let file_entry = self.find_in_directory(dir_cluster, filename[0])?;
        if file_entry.is_directory() {
            return Err(FSError::BadFS);
        }

        let mut cluster = file_entry.full_cluster();
        let mut total_read = 0;
        let cluster_size = self.bytes_per_sector as usize * self.sectors_per_cluster as usize;

        while cluster < 0x0FFFFFF8 && total_read < buf.len() {
            let mut cluster_buf = vec![0u8; cluster_size];
            self.read_cluster(cluster, &mut cluster_buf)?;

            let to_copy = (buf.len() - total_read).min(cluster_size);
            buf[total_read..total_read + to_copy].copy_from_slice(&cluster_buf[..to_copy]);
            total_read += to_copy;

            cluster = self.next_cluster(cluster)?;
        }

        return Ok(total_read.min(file_entry.size as usize));
    }

    fn write_file(&mut self, path: &str, buf: &[u8]) -> Result<(), FSError> {
        if !self.mounted {
            return Err(FSError::NotMounted);
        }

        let parts = split_path(path);
        if parts.is_empty() {
            return Err(FSError::BadPath);
        }

        let (dirs, filename) = parts.split_at(parts.len() - 1);
        let mut dir_cluster = self.root_cluster;

        for dir in dirs {
            let entry = self.find_in_directory(dir_cluster, dir)?;
            if !entry.is_directory() {
                return Err(FSError::NotADirectory);
            }
            dir_cluster = entry.full_cluster();
        }

        let short_name = Self::generate_8_3(filename[0]);
        let checksum = Self::lfn_checksum(&short_name);
        let chunks = Self::utf16_chunks(filename[0]);
        let entry_count = chunks.len();

        let mut dir_buf =
            vec![0u8; self.bytes_per_sector as usize * self.sectors_per_cluster as usize];
        self.read_cluster(dir_cluster, &mut dir_buf)?;

        let mut i = 0;
        while i + ((entry_count + 1) * 32) <= dir_buf.len() {
            let mut empty = true;
            for j in 0..(entry_count + 1) {
                if dir_buf[i + j * 32] != 0x00 && dir_buf[i + j * 32] != 0xE5 {
                    empty = false;
                    break;
                }
            }

            if empty {
                for (n, chunk) in chunks.iter().enumerate().rev() {
                    let entry_offset = i + (chunks.len() - 1 - n) * 32;
                    let mut e = [0u8; 32];

                    let seq = (n + 1) as u8;
                    e[0] = if n == chunks.len() - 1 {
                        0x40 | seq
                    } else {
                        seq
                    };
                    e[11] = 0x0F;
                    e[13] = checksum;

                    for k in 0..5 {
                        let bytes = chunk[k].to_le_bytes();
                        e[1 + k * 2] = bytes[0];
                        e[2 + k * 2] = bytes[1];
                    }
                    for k in 0..6 {
                        let bytes = chunk[5 + k].to_le_bytes();
                        e[14 + k * 2] = bytes[0];
                        e[15 + k * 2] = bytes[1];
                    }
                    for k in 0..2 {
                        let bytes = chunk[11 + k].to_le_bytes();
                        e[28 + k * 2] = bytes[0];
                        e[29 + k * 2] = bytes[1];
                    }

                    dir_buf[entry_offset..entry_offset + 32].copy_from_slice(&e);
                }

                let final_offset = i + entry_count * 32;
                let mut short = [0u8; 32];
                let cluster_size =
                    (self.bytes_per_sector * self.sectors_per_cluster as u16) as usize;
                let cluster_count = (buf.len() + cluster_size - 1) / cluster_size;
                let start_cluster = self.allocate_clusters(cluster_count)?;
                short[..11].copy_from_slice(&short_name);
                short[11] = 0x20;

                short[26] = 0x00;
                short[27] = 0x00;
                short[20] = 0x00;
                short[21] = 0x00;
                let size_bytes = (buf.len() as u32).to_le_bytes();
                short[28..32].copy_from_slice(&size_bytes);

                dir_buf[final_offset..final_offset + 32].copy_from_slice(&short);

                self.write_cluster(dir_cluster, &dir_buf)?;
                self.write_file_data(start_cluster, buf)?;

                return Ok(());
            }

            i += 32;
        }

        return Err(FSError::WriteError);
    }

    fn create_dir(&mut self, path: &str) -> Result<(), FSError> {
        if !self.mounted {
            return Err(FSError::NotMounted);
        }

        let parts = split_path(path);
        if parts.is_empty() {
            return Err(FSError::BadFS);
        }

        let (dirs, dirname) = parts.split_at(parts.len() - 1);
        let mut dir_cluster = self.root_cluster;

        for dir in dirs {
            let entry = self.find_in_directory(dir_cluster, dir)?;
            if !entry.is_directory() {
                return Err(FSError::BadFS);
            }
            dir_cluster = entry.full_cluster();
        }

        let new_cluster = self.allocate_clusters(1)?;

        self.init_directory_cluster(new_cluster, dir_cluster)?;

        self.insert_dir_entry(dir_cluster, dirname[0], new_cluster, true)?;

        return Ok(());
    }
}
