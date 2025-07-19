use crate::error;
use goblin::elf::program_header::PT_LOAD;
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB},
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

    for phdr in &elf.program_headers {
        if phdr.p_type != PT_LOAD {
            continue;
        }

        let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        if !phdr.is_executable() {
            flags |= PageTableFlags::NO_EXECUTE;
        }
        if phdr.is_write() {
            flags |= PageTableFlags::WRITABLE;
        }

        let vaddr = phdr.p_vaddr + 0x6000_0000_0000;

        let start_va = VirtAddr::new(vaddr);
        let end_va = VirtAddr::new(vaddr + phdr.p_memsz);

        let page_range = Page::range_inclusive(
            Page::containing_address(start_va),
            Page::containing_address(end_va - 1u64),
        );

        for page in page_range {
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
                    .expect("(ELF) Unable to map a page!")
                    .flush();
            }

            let virt_addr = page.start_address().as_u64() as *mut u8;
            unsafe {
                let dst = core::slice::from_raw_parts_mut(virt_addr, 4096);
                let page_offset =
                    page.start_address().as_u64() as usize - start_va.as_u64() as usize;

                let file_start = phdr.p_offset as usize + page_offset;
                let file_end =
                    (file_start + 4096).min(phdr.p_offset as usize + phdr.p_filesz as usize);
                let copy_len = file_end.saturating_sub(file_start);

                if copy_len > 0 {
                    dst[..copy_len].copy_from_slice(&bytes[file_start..file_end]);
                }

                if copy_len < 4096 {
                    dst[copy_len..].fill(0);
                }

                if !phdr.is_write() {
                    mapper
                        .update_flags(page, flags)
                        .expect("(ELF) Unable to update page's flags!")
                        .flush();
                }
            }
        }
    }

    return Some(VirtAddr::new(elf.entry + 0x6000_0000_0000));
}
