use alloc::{vec::*, vec, string::*};
use core::fmt::{self, Debug};
use super::FSError;
use efs::{error::Error, fs::error::FsError, io::{Base, Read, Seek, SeekFrom, Write}};

// Implement Error trait for FSError
impl core::error::Error for FSError {}

impl fmt::Display for FSError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Filesystem error")
    }
}

#[derive(Debug)]
pub struct BlockDev {
    data: Vec<u8>,
    pos: usize,
}

impl BlockDev {
    pub fn new(size: usize) -> BlockDev {
        BlockDev {
            data: vec![0; size],
            pos: 0,
        }
    }

    pub fn from_data(data: Vec<u8>) -> Self {
        return Self {
            data,
            pos: 0,
        }
    }
}

impl Base for BlockDev {
    type FsError = FSError;
}

impl Read for BlockDev {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error<FSError>> {
        let available = self.data.len() - self.pos;
        let len = buf.len().min(available);
        if len == 0 {
            return Ok(0);
        }
        buf[..len].copy_from_slice(&self.data[self.pos..self.pos + len]);
        self.pos += len;
        Ok(len)
    }
}

impl Write for BlockDev {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error<FSError>> {
        let available = self.data.len() - self.pos;
        let len = buf.len().min(available);
        if len == 0 {
            return Ok(0);
        }
        self.data[self.pos..self.pos + len].copy_from_slice(&buf[..len]);
        self.pos += len;
        Ok(len)
    }
    
    fn flush(&mut self) -> Result<(), Error<FSError>> {
        Ok(())
    }
}

impl Seek for BlockDev {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Error<FSError>> {
        self.pos = match pos {
            SeekFrom::Start(offset) => {
                if offset > self.data.len() as u64 {
                    return Err(FsError::NotFound("Cannot seek beyond end of file".to_string()).into());
                }
                offset as usize
            }
            SeekFrom::End(offset) => {
                let end_pos = self.data.len() as isize + offset as isize;
                if end_pos < 0 {
                    return Err(FsError::NotFound("Cannot seek before start of file".to_string()).into());
                }
                end_pos as usize
            }
            SeekFrom::Current(offset) => {
                let current_pos = self.pos as isize + offset as isize;
                if current_pos < 0 || current_pos > self.data.len() as isize {
                    return Err(FsError::NotFound("Cannot seek beyond file bounds".to_string()).into());
                }
                current_pos as usize
            }
        };
        return Ok(self.pos as u64);
    }
}

pub fn mount_ext2<D: Read + Write + Seek + Debug>(mut dev: D) -> Result<(), Error<FSError>> {
    // Read the superblock
    let mut superblock = [0u8; 1024];
    dev.seek(SeekFrom::Start(1024)).expect("Couldn't seek to superblock!");
    dev.read_exact(&mut superblock).expect("Unable to read superblock!");

    // Check the magic number
    if u16::from_le_bytes(superblock[56..58].try_into().unwrap()) != 0xef53 {
        return Err(FsError::NotFound("Not an ext2 filesystem".to_string()).into());
    }

    // Read the block size
    let block_size = 1024 << (superblock[24] as usize);
    
    // Read the inode table
    let inode_table_offset = u32::from_le_bytes(superblock[40..44].try_into().unwrap()) as usize * block_size;
    dev.seek(SeekFrom::Start(inode_table_offset as u64)).expect("Couldn't seek to inode table!");
    
    // Read the inodes
    let mut inodes = vec![0u8; block_size];
    dev.read_exact(&mut inodes).expect("Unable to read inodes!");

    // Process inodes...
    
    Ok(())
}