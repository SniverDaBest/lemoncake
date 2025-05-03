use crate::{ahci::AhciDevice, info};
use super::{Filesystem, FSError};

#[derive(Debug)]
#[repr(C, packed)]
struct SHFSHeader {
	signature: [u8; 5],
	name: [u8; 16],
	rev: u8,
	piece_sz: u16,
	piece_count: u64,
	free_space: u64,
	index_end: u64,
}

#[derive(Debug)]
pub struct SHFS {
    header: SHFSHeader,
    device: AhciDevice,
}

impl SHFS {
    
}

impl Filesystem for SHFS {
    fn mount(&mut self) -> Result<(), FSError> {
        info!("Mounting SHFS filesystem.");
        return Err(FSError::MountError);
    }

    fn unmount(&mut self) -> Result<(), FSError> {
        info!("Unmounting SHFS filesystem.");
        return Err(FSError::UnmountError);
    }

    /// Returns how much data was written into the buffer if successful.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FSError> {
        return Err(FSError::ReadError);
    }

    fn write(&mut self, buf: &[u8]) -> Result<(), FSError> {
        return Err(FSError::WriteError);
    }
}