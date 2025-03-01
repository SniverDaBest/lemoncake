#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

pub mod allocator;
#[macro_use]
pub mod vga;
pub mod acpi;
use core::panic::PanicInfo;
use core::alloc::Layout;
use allocator::BootInfoFrameAllocator;
use multiboot2::{BootInformation, BootInformationHeader, MemoryArea, MemoryMapTag};
use acpi::*;
use x86_64::VirtAddr;
pub const PHYSICAL_MEMORY_OFFSET: VirtAddr = VirtAddr::new(0x100000);

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main(mbi: u32, magic: u32) -> ! {
    println!("You should star SniverDaBest/lemoncake!");
    if magic != multiboot2::MAGIC {
        panic!("magic given was NOT the correct magic!");
    }
    let boot_info: BootInformation<'static> = unsafe { BootInformation::load(mbi as *const BootInformationHeader) }.expect("Unable to get MB2 boot info!");

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