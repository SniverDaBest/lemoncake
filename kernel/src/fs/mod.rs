//pub mod ext2;
pub mod shfs;

#[derive(Debug)]
pub enum FSError {
    ReadError,
    WriteError,
    MountError,
    UnmountError,
}

pub trait Filesystem {
    fn mount(&mut self) -> Result<(), FSError>;
    fn unmount(&mut self) -> Result<(), FSError>;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FSError>;
    fn write(&mut self, buf: &[u8]) -> Result<(), FSError>;
}