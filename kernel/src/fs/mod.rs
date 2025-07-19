//pub mod fat;
//pub mod sfs;
use alloc::vec::Vec;
use core::fmt::{self, Display, Formatter};

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

impl Display for FSError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::ReadError => write!(f, "Read Error"),
            Self::WriteError => write!(f, "Write Error"),
            Self::MountError => write!(f, "Mount Error"),
            Self::UnmountError => write!(f, "Unmount Error"),
            Self::BadFS => write!(f, "Bad File System"),
            Self::NotMounted => write!(f, "File System Not Mounted"),
            Self::AlreadyMounted => write!(f, "File System Already Mounted"),
            Self::NotADirectory => write!(f, "Not a Directory"),
            Self::NotAFile => write!(f, "Not a File"),
            Self::BadPath => write!(f, "Bad Path"),
            Self::NotFound => write!(f, "Not Found"),
            Self::AlreadyExists => write!(f, "Already Exists"),
            Self::BadFile => write!(f, "Bad File"),
            Self::BadDirectory => write!(f, "Bad Directory"),
            Self::NoSpace => write!(f, "No Space Available"),
            Self::NameTooLong => write!(f, "Name Too Long"),
            Self::DirectoryNotEmpty => write!(f, "Directory Not Empty"),
        }
    }
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
