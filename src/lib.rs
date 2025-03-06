#![no_std]
#![no_main]
#![feature(alloc_error_handler, abi_x86_interrupt)]
#![allow(static_mut_refs)]

extern crate alloc;

pub mod allocator;
#[macro_use]
pub mod vga;
pub mod acpi;
pub mod base64;
pub mod command_line;
pub mod executor;
pub mod fs;
pub mod interrupts;
pub mod keyboard;
pub mod disks;
pub mod pci;
use alloc::string::*;
use allocator::BootInfoFrameAllocator;
use core::alloc::Layout;
use core::panic::PanicInfo;
use executor::{Executor, Task};
use multiboot2::{BootInformation, BootInformationHeader};
use x86_64::{VirtAddr, instructions::port::{Port, PortRead}};
pub const PHYSICAL_MEMORY_OFFSET: VirtAddr = VirtAddr::new(0x0);
pub const LEMONCAKE_VER: &str = "25m3";

#[cfg(not(target_arch = "x86_64"))]
compile_error!("This OS only supports x86_64!");

#[unsafe(no_mangle)]
pub extern "C" fn kernel_main(mbi: u32, magic: u32) -> ! {
    info!("Running Lemoncake {}", LEMONCAKE_VER);
    if magic != multiboot2::MAGIC {
        panic!("MB2 magic given was NOT the correct magic! Expected {:#X?}, but got {:#X?}!", multiboot2::MAGIC, magic);
    }

    info!("Setting up IDT, PICS, and interrupts...");
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();

    let boot_info = unsafe { BootInformation::load(mbi as *const BootInformationHeader) }
        .expect("Unable to get MB2 boot info!");
    let boot_info: &'static BootInformation = unsafe { &*(&boot_info as *const _) };

    let memory_map_tag = boot_info.memory_map_tag().expect("Memory map tag required");
    let memory_areas = memory_map_tag.memory_areas();
    let mut total_memory: usize = 0;
    for area in memory_areas
        .iter()
        .filter(|area| area.typ() == multiboot2::MemoryAreaType::Available)
    {
        total_memory += area.size() as usize;
    }
    if total_memory < 128 * 1000 * 1000 {
        panic!("You must have at least 128MB of memory available to run Lemoncake!");
    }
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(memory_areas) };
    let mut mapper = unsafe { allocator::init_mapper(PHYSICAL_MEMORY_OFFSET) };
    info!("Initializing heap... This may take a while!");
    allocator::init_heap(&mut mapper, &mut frame_allocator, memory_areas)
        .expect("Heap initialization failed");

    info!("All done!");

    let mut executor = Executor::new();
    executor.spawn(Task::new(command_line::run_command_line()));
    executor.run();
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    crate::vga::set_fg(crate::vga::Color::Red);

    println!(
        "(X_X)\n\nUh-oh! Lemoncake panicked. This usually means that something super bad happened.\n\nMessage: {}\nLocation: {}",
        info.message(),
        info.location().unwrap()
    );

    loop {}
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    crate::vga::set_fg(crate::vga::Color::Red);
    println!(
        "(X_X)\n\nUh-oh! Lemoncake panicked, as it was unable to allocate {} bytes of memory!\n\nLayout: {:?}",
        layout.size(),
        layout
    );

    loop {}
}

pub fn bool_to_yn(var: bool) -> String {
    match var {
        true => "yes".to_string(),
        false => "no".to_string(),
    }
}

pub unsafe fn read_from_port<T: PortRead>(port: u16) -> T {
    let mut p: Port<T> = Port::new(port);
    unsafe { return p.read(); }
}

pub unsafe fn write_to_port(port: u16, data: u32) {
    let mut p: Port<u32> = Port::new(port);
    unsafe { p.write(data); }
}

/// Writes an 8-bit value to the specified I/O port.
pub unsafe fn outb(port: u16, value: u8) {
    unsafe { write_to_port(port, value as u32); }
}

/// Reads an 8-bit value from the specified I/O port.
pub unsafe fn inb(port: u16) -> u8 {
    unsafe { return read_from_port(port); }
}

/// Reads a 32-bit value from the specified I/O port.
#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    unsafe { return read_from_port(port); }
}

/// Writes a 32-bit value to the specified I/O port.
#[inline]
pub unsafe fn outl(port: u16, val: u32) {
    unsafe { write_to_port(port, val); }
}

#[macro_export]
macro_rules! nftodo {
    () => {
        let prev = $crate::vga::get_colors()[1];
        $crate::vga::set_fg($crate::vga::Color::Pink);
        println!("todo");
        $crate::vga::set_fg(prev);
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        let prev_fg = $crate::vga::get_colors()[1];
        $crate::vga::set_fg($crate::vga::Color::Red);
        ($crate::println!("(o_0)  Error: {}", format_args!($($arg)*)));
        $crate::vga::set_fg(prev_fg);
    }
}

#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => {
        let prev_fg = $crate::vga::get_colors()[1];
        $crate::vga::set_fg($crate::vga::Color::Yellow);
        ($crate::println!("(o_0)  Warning: {}", format_args!($($arg)*)));
        $crate::vga::set_fg(prev_fg);
    }
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        let prev_fg = $crate::vga::get_colors()[1];
        $crate::vga::set_fg($crate::vga::Color::LightBlue);
        ($crate::println!("(o_o)  Info: {}", format_args!($($arg)*)));
        $crate::vga::set_fg(prev_fg);
    }
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt()
    }
}
