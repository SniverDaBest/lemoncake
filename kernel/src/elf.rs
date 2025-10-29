use crate::{
    allocator::{alloc_pages_user, alloc_user},
    error,
    syscall::jump_to_usermode,
};
use core::{fmt, slice};
use goblin::elf::program_header::PT_LOAD;
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, Size4KiB},
};

pub struct Process {
    #[allow(unused)]
    pid: Option<u64>,
    stack_addr: Option<u64>,
    data: &'static [u8],
    entrypoint: Option<u64>,
}

#[derive(Debug)]
pub enum ProcInitError {
    AllocationError,
    InvalidElf,
}

impl fmt::Display for ProcInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationError => write!(f, "Allocation Error"),
            Self::InvalidElf => write!(f, "Invalid Elf"),
            #[allow(unreachable_patterns)]
            u => write!(f, "{:?}", u),
        }
    }
}

impl Process {
    pub fn new(data: &'static [u8]) -> Self {
        return Self {
            pid: None,
            stack_addr: None,
            data,
            entrypoint: None,
        };
    }

    pub fn init(
        &mut self,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<(), ProcInitError> {
        let elf = match goblin::elf::Elf::parse(self.data) {
            Ok(e) => e,
            Err(e) => {
                error!("(ELF) Invalid ELF: {}", e);
                return Err(ProcInitError::InvalidElf);
            }
        };

        let mut req_sz = 0u64;

        for phdr in &elf.program_headers {
            if phdr.p_type != PT_LOAD {
                continue;
            }
            if req_sz < phdr.p_vaddr {
                req_sz += phdr.p_vaddr;
            }
            req_sz += phdr.p_memsz;
        }

        let user_base = match alloc_user(mapper, frame_allocator, req_sz as usize) {
            Some(a) => a.as_u64(),
            None => {
                error!("(ELF) Unable to allocate {} bytes for executable!", req_sz);
                return Err(ProcInitError::AllocationError);
            }
        };

        for phdr in &elf.program_headers {
            if phdr.p_type != PT_LOAD {
                continue;
            }

            let dst = unsafe {
                slice::from_raw_parts_mut(
                    (user_base + phdr.p_vaddr) as *mut u8,
                    phdr.p_memsz as usize,
                )
            };

            let file_range = phdr.p_offset as usize..(phdr.p_offset + phdr.p_filesz) as usize;
            let src = &self.data[file_range];

            let copy_len = src.len().min(dst.len());

            dst[..copy_len].copy_from_slice(&src[..copy_len]);
            if copy_len < dst.len() {
                dst[copy_len..].fill(0);
            }
        }

        self.entrypoint = Some(elf.entry + user_base);
        return Ok(());
    }

    pub fn alloc_stack(
        &mut self,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        pages: usize,
    ) -> Option<VirtAddr> {
        return alloc_pages_user(mapper, frame_allocator, pages);
    }

    pub unsafe fn switch(
        &mut self,
        pages: usize,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) {
        if self.entrypoint.is_none() {
            match self.init(mapper, frame_allocator) {
                Ok(_) => {}
                Err(e) => {
                    error!("(ELF) Unable to initialize executable! Error: {}", e);
                    return;
                }
            };
        }

        if self.stack_addr.is_none() {
            self.stack_addr = match self.alloc_stack(mapper, frame_allocator, pages) {
                Some(s) => Some(s.as_u64()),
                None => {
                    error!("(ELF) Unable to allocate stack for executable!");
                    return;
                }
            };
        }

        jump_to_usermode(self.entrypoint.unwrap(), self.stack_addr.unwrap());
    }
}
