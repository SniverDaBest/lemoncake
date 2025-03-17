#![no_std]
#![no_main]
#![feature(alloc_error_handler, abi_x86_interrupt)]
#![allow(static_mut_refs)]

extern crate alloc;
pub mod base64;
pub mod display;
pub mod executor;
pub mod fonts;

/*
pub mod interrupts;
pub mod acpi;
pub mod disks;
pub mod fs;
pub mod pci;
pub mod command_line;
pub mod keyboard;
use allocator::BootInfoFrameAllocator;
use core::alloc::Layout;
use x86_64::{
    VirtAddr,
    instructions::port::{Port, PortRead},
};
pub const PHYSICAL_MEMORY_OFFSET: VirtAddr = VirtAddr::new(0x0);
*/

pub const LEMONCAKE_VER: &str = "25m3-UEFI";
use core::mem::MaybeUninit;

use display::Buffer;
use alloc::vec;
use log::{error, info};
use uefi::{
    helpers,
    prelude::*,
    proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput, Mode, ModeIter},
};

fn get_good_mode(modes: ModeIter) -> Mode {
    for m in modes {
        if m.info().resolution() == (640, 480) {
            info!("Found good mode:\n{:#?}", m);
            return m;
        }
    }

    panic!("Couldn't find a good mode!");
}

#[entry]
fn main() -> Status {
    helpers::init().unwrap();

    let gop_handle =
        boot::get_handle_for_protocol::<GraphicsOutput>().expect("Unable to find the GOP!");
    let mut gop = boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle).expect("Unable to get the GOP!");
    let mode = get_good_mode(gop.modes());

    gop.set_mode(&mode).expect("Unable to set GOP mode!");

    loop {}
    
    return Status::SUCCESS;
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!(
        "(X_X)\n\nUh-Oh, Lemoncake panicked!\nMessage: {}\nLocation: {}@L{}:{}",
        info.message(),
        info.location().unwrap().file(),
        info.location().unwrap().line(),
        info.location().unwrap().column()
    );

    loop {}
}

/*
#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!(
        "(X_X)\n\nUh-oh! Lemoncake panicked, as it was unable to allocate {} bytes of memory!\n\nLayout: {:?}",
        layout.size(),
        layout
    );
}

pub fn bool_to_yn(var: bool) -> String {
    match var {
        true => "yes".to_string(),
        false => "no".to_string(),
    }
}

pub unsafe fn read_from_port<T: PortRead>(port: u16) -> T {
    let mut p: Port<T> = Port::new(port);
    unsafe {
        return p.read();
    }
}

pub unsafe fn write_to_port(port: u16, data: u32) {
    let mut p: Port<u32> = Port::new(port);
    unsafe {
        p.write(data);
    }
}

/// Writes an 8-bit value to the specified I/O port.
pub unsafe fn outb(port: u16, value: u8) {
    unsafe {
        write_to_port(port, value as u32);
    }
}

/// Reads an 8-bit value from the specified I/O port.
pub unsafe fn inb(port: u16) -> u8 {
    unsafe {
        return read_from_port(port);
    }
}

/// Reads a 32-bit value from the specified I/O port.
#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    unsafe {
        return read_from_port(port);
    }
}

/// Writes a 32-bit value to the specified I/O port.
#[inline]
pub unsafe fn outl(port: u16, val: u32) {
    unsafe {
        write_to_port(port, val);
    }
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
*/
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt()
    }
}
