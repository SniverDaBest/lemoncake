use alloc::alloc::{GlobalAlloc, Layout};
use bump::BumpAllocator;
use core::ptr::null_mut;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame, Size4KiB,
        mapper::{MapToError, UnmapError},
    },
};

pub mod bump;
pub mod fixed_size_block;
pub mod linked_list;

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 15 * 1024 * 1024; // 15 MiB

#[global_allocator]
static ALLOCATOR: Locked<BumpAllocator> = Locked::new(BumpAllocator::new());

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() };
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

pub struct Dummy;

unsafe impl GlobalAlloc for Dummy {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("dealloc should be never called")
    }
}

/// A wrapper around spin::Mutex to permit trait implementations.
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    #[allow(mismatched_lifetime_syntaxes)]
    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}

/// Align the given address `addr` upwards to alignment `align`.
///
/// Requires that `align` is a power of two.
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

pub fn alloc_pages(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    num_pages: usize,
) -> Option<VirtAddr> {
    static mut NEXT_VIRT: u64 = 0xffff_9000_0000_0000;
    let base;
    unsafe {
        base = NEXT_VIRT;
        NEXT_VIRT += (num_pages as u64) * 4096;
    }

    for i in 0..num_pages {
        let virt = VirtAddr::new(base + (i as u64) * 4096);
        let phys = frame_allocator
            .allocate_frame()
            .expect("(ALLOCATOR) Unable to allocate a frame!");
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper
                .map_to(Page::containing_address(virt), phys, flags, frame_allocator)
                .expect("(ALLOCATOR) Unable to map a page!")
                .flush();
        }
    }
    Some(VirtAddr::new(base))
}

pub fn alloc_pages_user(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    num_pages: usize,
) -> Option<VirtAddr> {
    static mut NEXT_VIRT: u64 = 0xffff_ffff_9000_0000;
    let base;
    unsafe {
        base = NEXT_VIRT;
        NEXT_VIRT += (num_pages as u64) * 4096;
    }

    for i in 0..num_pages {
        let virt = VirtAddr::new(base + (i as u64) * 4096);
        let phys = frame_allocator
            .allocate_frame()
            .expect("(ALLOCATOR) Unable to allocate a frame!");
        let flags =
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
        unsafe {
            mapper
                .map_to(Page::containing_address(virt), phys, flags, frame_allocator)
                .expect("(ALLOCATOR) Unable to map a page!")
                .flush();
        }
    }
    Some(VirtAddr::new(base))
}

pub fn map_page(
    mapper: &mut impl Mapper<Size4KiB>,
    virt_addr: u64,
    phys_addr: u64,
    flags: PageTableFlags,
) -> Result<(), MapToError<Size4KiB>> {
    let page = Page::containing_address(VirtAddr::new(virt_addr));
    let frame = PhysFrame::containing_address(PhysAddr::new(phys_addr));
    unsafe {
        mapper
            .map_to(page, frame, flags, &mut crate::memory::EmptyFrameAllocator)?
            .flush();
    }
    Ok(())
}

pub fn unmap_page(mapper: &mut impl Mapper<Size4KiB>, virt_addr: u64) -> Result<(), UnmapError> {
    let page = Page::containing_address(VirtAddr::new(virt_addr));
    mapper.unmap(page)?.1.flush();
    return Ok(());
}

pub fn alloc(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    num_bytes: usize,
) -> Option<VirtAddr> {
    if num_bytes == 0 {
        return None;
    }

    return alloc_pages(mapper, frame_allocator, num_bytes.div_ceil(4096).max(2));
}

pub fn alloc_user(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    num_bytes: usize,
) -> Option<VirtAddr> {
    if num_bytes == 0 {
        return None;
    }

    return alloc_pages_user(mapper, frame_allocator, num_bytes.div_ceil(4096).max(2));
}

pub fn free(
    mapper: &mut impl Mapper<Size4KiB>,
    virt_addr: VirtAddr,
    num_bytes: usize,
) -> Result<(), UnmapError> {
    if virt_addr.is_null() || num_bytes == 0 {
        return Ok(());
    }

    let num_pages = num_bytes.div_ceil(4096);

    for i in 0..num_pages {
        let virt = virt_addr + i as u64 * 4096;
        let page = Page::containing_address(virt);
        mapper.unmap(page)?.1.flush();
    }

    return Ok(());
}
