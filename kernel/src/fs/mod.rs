pub mod fat;

#[derive(Debug)]
pub enum FSError {
    ReadError,
    WriteError,
    MountError,
    UnmountError,
    BadFS,
    NotMounted,
    AlreadyMounted,
    NotADirectory,
    NotAFile,
    BadPath,
}

pub trait Filesystem {
    fn mount(&mut self) -> Result<(), FSError>;
    fn unmount(&mut self) -> Result<(), FSError>;
    fn read_file(&mut self, path: &str, buf: &mut [u8]) -> Result<usize, FSError>;
    fn write_file(&mut self, path: &str, buf: &[u8]) -> Result<(), FSError>;
    fn create_dir(&mut self, path: &str) -> Result<(), FSError>;
}
