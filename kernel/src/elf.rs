use core::slice;

use crate::{allocator::alloc_user, error, info};
use goblin::elf::program_header::PT_LOAD;
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, Size4KiB},
};

pub fn load_elf(
    bytes: &[u8],
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Option<VirtAddr> {
    let elf = match goblin::elf::Elf::parse(bytes) {
        Ok(e) => e,
        Err(e) => {
            error!("(ELF) Invalid ELF: {}", e);
            return None;
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
        Some(a) => {
            info!("(ELF) Allocated {} bytes for executable!", req_sz);
            a.as_u64()
        }
        None => {
            error!("(ELF) Unable to allocate {} bytes for executable!", req_sz);
            return None;
        }
    };

    for phdr in &elf.program_headers {
        if phdr.p_type != PT_LOAD {
            continue;
        }

        let dst = unsafe {
            slice::from_raw_parts_mut((user_base + phdr.p_vaddr) as *mut u8, phdr.p_memsz as usize)
        };

        let file_range = phdr.p_offset as usize..(phdr.p_offset + phdr.p_filesz) as usize;
        let src = &bytes[file_range];

        let copy_len = src.len().min(dst.len());

        dst[..copy_len].copy_from_slice(&src[..copy_len]);
        if copy_len < dst.len() {
            dst[copy_len..].fill(0);
        }
    }

    return Some(VirtAddr::new(elf.entry + user_base));
}
