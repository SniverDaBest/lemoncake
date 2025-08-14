use crate::error;
use goblin::elf::program_header::PT_LOAD;
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB},
};

const USER_BASE: u64 = 0x0000_6000_0000;

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

    for phdr in &elf.program_headers {
        if phdr.p_type != PT_LOAD {
            continue;
        }

        let seg_start = phdr.p_vaddr + USER_BASE;
        let seg_end = seg_start + phdr.p_memsz;

        let start_page = Page::containing_address(VirtAddr::new(seg_start));
        let end_page = Page::containing_address(VirtAddr::new(seg_end - 1));

        let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        if !phdr.is_executable() {
            flags |= PageTableFlags::NO_EXECUTE;
        }
        if phdr.is_write() {
            flags |= PageTableFlags::WRITABLE;
        }

        for page in Page::range_inclusive(start_page, end_page) {
            let frame = frame_allocator
                .allocate_frame()
                .expect("(ELF) Could not allocate frame");

            unsafe {
                mapper
                    .map_to(
                        page,
                        frame,
                        flags | PageTableFlags::WRITABLE,
                        frame_allocator,
                    )
                    .expect("(ELF) Unable to map page")
                    .flush();
            }

            let virt_addr = page.start_address().as_u64() as *mut u8;
            let dst = unsafe { core::slice::from_raw_parts_mut(virt_addr, 4096) };

            let page_offset = page.start_address().as_u64() as usize - seg_start as usize;

            let file_start = phdr.p_offset as usize + page_offset;
            let file_end = (file_start + 4096).min(phdr.p_offset as usize + phdr.p_filesz as usize);

            if file_start < file_end && file_start < bytes.len() {
                let copy_len = file_end - file_start;
                dst[..copy_len].copy_from_slice(&bytes[file_start..file_end]);
                if copy_len < 4096 {
                    dst[copy_len..].fill(0);
                }
            } else {
                dst.fill(0);
            }

            if !phdr.is_write() {
                unsafe {
                    mapper
                        .update_flags(page, flags)
                        .expect("(ELF) Unable to update flags")
                        .flush();
                }
            }
        }
    }

    return Some(VirtAddr::new(elf.entry + USER_BASE));
}
