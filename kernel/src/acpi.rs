use acpi::{AcpiHandler, PhysicalMapping};
use core::ptr::NonNull;
use x86_64::{
    structures::paging::{
        mapper::MapperFlush, FrameAllocator, Mapper, Page, PageSize, PageTableFlags, PhysFrame, Size4KiB
    },
    PhysAddr, VirtAddr,
};
use alloc::vec::Vec;
use crate::info;
use core::cell::RefCell;

#[derive(Debug, Clone)]
pub struct PagingAcpiHandler<M, A> {
    mapper: RefCell<M>,
    frame_allocator: RefCell<A>,
}

impl<M, A> PagingAcpiHandler<M, A>
where
    M: Mapper<Size4KiB>,
    A: FrameAllocator<Size4KiB>,
{
    pub fn new(mapper: M, frame_allocator: A) -> Self {
        Self {
            mapper: RefCell::new(mapper),
            frame_allocator: RefCell::new(frame_allocator),
        }
    }
}

impl<M, A> AcpiHandler for PagingAcpiHandler<M, A>
where
    M: Mapper<Size4KiB> + Clone,
    A: FrameAllocator<Size4KiB> + Clone,
{
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        info!("Mapping {} bytes at phys 0x{:X}.", size, physical_address);

        let page_count = (size + Size4KiB::SIZE as usize - 1) / Size4KiB::SIZE as usize;

        let pages: Vec<Page<Size4KiB>> = (0..page_count)
            .map(|i| {
                let addr = physical_address as u64 + (i * Size4KiB::SIZE as usize) as u64;
                Page::containing_address(VirtAddr::new(addr))
            })
            .collect();

        for page in &pages {
            let frame = PhysFrame::containing_address(PhysAddr::new(
                physical_address as u64 + (page.start_address().as_u64() - physical_address as u64),
            ));
            let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
            let flush: MapperFlush<Size4KiB> = self
                .mapper
                .borrow_mut()
                .map_to(*page, frame, flags, &mut *self.frame_allocator.borrow_mut())
                .expect("map_to failed");                     
        }

        let virt_ptr = physical_address as *mut T;

        PhysicalMapping::new(
            physical_address,
            NonNull::new_unchecked(virt_ptr),
            size,
            page_count * Size4KiB::SIZE as usize,
            self.clone(),
        )
    }

    fn unmap_physical_region<T>(region: &PhysicalMapping<Self, T>) {
        let virt_start = region.virtual_start().as_ptr() as u64;
        let pages = (region.region_length() + Size4KiB::SIZE as usize - 1)
            / Size4KiB::SIZE as usize;

        for i in 0..pages {
            let page = Page::containing_address(VirtAddr::new(
                virt_start + (i as u64 * Size4KiB::SIZE),
            ));
            // Unmap returns (PhysFrame, MapperFlush)
            let ( _frame, flush ) = region.handler()
                    .mapper
                    .borrow_mut()
                    .unmap(page)
                    .expect("unmap failed");
            flush.flush();
        }
    }
}
