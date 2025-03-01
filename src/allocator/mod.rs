use alloc::alloc::{GlobalAlloc, Layout};
use fixed_size_block::FixedSizeBlockAllocator;
use core::ptr::null_mut;
use x86_64::{
    VirtAddr,
    PhysAddr,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB, mapper::MapToError, OffsetPageTable, PageTable, PhysFrame
    }, registers::control::Cr3,
};
use multiboot2::MemoryArea;

pub mod bump;
pub mod fixed_size_block;
pub mod linked_list;

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());

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

/// Initializes a new OffsetPageTable.
///
/// # Safety
/// The caller must guarantee that the complete physical memory is mapped at the passed `physical_memory_offset`.
pub unsafe fn init_mapper(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let level_4_table_ptr: *mut PageTable = virt.as_mut_ptr();
    let level_4_table: &mut PageTable = &mut *level_4_table_ptr;
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// A frame allocator that returns usable frames from the multiboot2 memory map.
pub struct BootInfoFrameAllocator {
    memory_areas: &'static [MemoryArea],
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Create a frame allocator from the passed memory map.
    ///
    /// # Safety
    /// The caller must guarantee that the passed memory map is valid.
    pub unsafe fn init(memory_areas: &'static [MemoryArea]) -> Self {
        BootInfoFrameAllocator {
            memory_areas,
            next: 0,
        }
    }

    /// Returns an iterator over usable physical frames.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame<Size4KiB>> {
        self.memory_areas.iter().filter(|area| area.typ() == multiboot2::MemoryAreaType::Available)
            .flat_map(|area| {
                let start_frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(PhysAddr::new(area.start_address()));
                // Note: area.length might not be an exact multiple of 4096, so you may want to adjust.
                let end_frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(PhysAddr::new(area.start_address() + area.size()));
                (start_frame.start_address().as_u64()/4096..end_frame.start_address().as_u64()/4096)
                    .map(|frame_number| PhysFrame::from_start_address(PhysAddr::new(frame_number * 4096)).unwrap())
            })
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}