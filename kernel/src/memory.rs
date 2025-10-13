use limine::memory_map::{Entry, EntryType};
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, OffsetPageTable, PageSize, PageTable, PhysFrame, Size4KiB,
    },
};

pub unsafe fn init(hhdm_offset: VirtAddr) -> OffsetPageTable<'static> {
    let lvl4 = active_level_4_table(hhdm_offset);
    OffsetPageTable::new(lvl4, hhdm_offset)
}

unsafe fn active_level_4_table(hhdm_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_frame, _) = Cr3::read();

    let phys = level_4_frame.start_address();
    let virt = hhdm_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

/// A FrameAllocator that always returns `None`.
pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}

#[derive(Clone)]
/// A FrameAllocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    entries: &'static [&'static Entry],
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(entries: &'static [&'static Entry]) -> Self {
        BootInfoFrameAllocator { entries, next: 0 }
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        self.entries
            .iter()
            .filter(|r| r.entry_type == EntryType::USABLE)
            .flat_map(|r| {
                let start = r.base;
                let end = r.base + r.length;
                (start..end).step_by(Size4KiB::SIZE as usize)
            })
            .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
