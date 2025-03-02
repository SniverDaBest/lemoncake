#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

pub mod allocator;
#[macro_use]
pub mod vga;
pub mod acpi;
pub mod fs;
use core::panic::PanicInfo;
use core::alloc::Layout;
use allocator::BootInfoFrameAllocator;
use multiboot2::{BootInformation, BootInformationHeader, FramebufferTag};
use x86_64::VirtAddr;
pub const PHYSICAL_MEMORY_OFFSET: VirtAddr = VirtAddr::new(0x0);

#[macro_export]
macro_rules! nftodo {
    () => {
        println!("todo");
    };
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main(mbi: u32, magic: u32) -> ! {
    println!("You should star SniverDaBest/lemoncake!");
    if magic != multiboot2::MAGIC {
        panic!("magic given was NOT the correct magic!");
    }
    let boot_info = unsafe { BootInformation::load(mbi as *const BootInformationHeader) }.expect("Unable to get MB2 boot info!");
    let boot_info: &'static BootInformation = unsafe { &*(&boot_info as *const _) };

    match boot_info.framebuffer_tag().unwrap() {
        Ok(tag) => println!("Found framebuffer at {:#X?}!", tag.address()),
        Err(e) => panic!("Unable to access Framebuffer Tag! Error: {}", e),
    }

    let fb_tag = boot_info.framebuffer_tag().unwrap().unwrap();
    match fb_tag.buffer_type() {
        Ok(typ) => println!("Framebuffer type: {:?}", typ),
        Err(e) => panic!("Couldn't get framebuffer type! Error: {}", e)
    }

    // Now borrow the memory map tag:
    let memory_map_tag = boot_info.memory_map_tag().expect("Memory map tag required");
    let memory_areas = memory_map_tag.memory_areas();
    let mut total_memory: usize = 0;
    for area in memory_areas.iter().filter(|area| area.typ() == multiboot2::MemoryAreaType::Available) {
        total_memory += area.size() as usize;
    }
    if total_memory < 128*1000*1000 {
        panic!("You must have at least 128MB of memory available to run Lemonade!");
    }
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(memory_areas) };
    // Initialize your heap here:
    let mut mapper = unsafe { allocator::init_mapper(PHYSICAL_MEMORY_OFFSET) };
    println!("Initializing heap... This may take a while!");
    allocator::init_heap(&mut mapper, &mut frame_allocator, memory_areas)
        .expect("Heap initialization failed");

    println!("All done!");

    loop {}
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    println!(
        "(X_X)\n\nUh-oh! Lemoncake panicked. This usually means that something super bad happened.\n\nMessage: {}\nLocation: {}",
        _info.message(),
        _info.location().unwrap()
    );

    loop {}
}

#[alloc_error_handler]
fn alloc_error_handler(_layout: Layout) -> ! {
    println!(
        "(X_X)\n\nUh-oh! Lemoncake panicked, as it was unable to allocate {} bytes of memory!\n\nLayout: {:?}", _layout.size(), _layout
    );
    
    loop {}
}