//pub mod fat;
pub mod sfs;
use alloc::vec::Vec;

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
    NotFound,
    AlreadyExists,
    BadFile,
    BadDirectory,
    NoSpace,
    NameTooLong,
    DirectoryNotEmpty,
}

pub type FSResult<T> = Result<T, FSError>;

pub trait Filesystem {
    fn mount(&mut self) -> Result<(), FSError>;
    fn unmount(&mut self) -> Result<(), FSError>;
    fn read_file(&mut self, path: &str, buf: &mut [u8]) -> Result<usize, FSError>;
    fn write_file(&mut self, path: &str, buf: &[u8]) -> Result<(), FSError>;
    fn remove_file(&mut self, path: &str) -> Result<(), FSError>;
    fn create_dir(&mut self, path: &str) -> Result<(), FSError>;
    fn remove_dir(&mut self, path: &str) -> Result<(), FSError>;
}

pub fn split_path(path: &str) -> Vec<&str> {
    return path
        .trim_start_matches('/')
        .split('/')
        .filter(|p| !p.is_empty())
        .collect();
}
