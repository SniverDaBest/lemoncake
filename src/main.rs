#![no_std]
#![no_main]
#![feature(alloc_error_handler, abi_x86_interrupt, custom_test_frameworks)]
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

use alloc::{string::*, vec::*};
use display::Buffer;
use fontdue::{Font, FontSettings};
use log::{error, info};
use uefi::{
    CString16,
    fs::{FileSystem, FileSystemResult},
    helpers,
    prelude::*,
    proto::console::gop::{BltPixel, GraphicsOutput, Mode, ModeIter},
};

fn get_good_mode(modes: ModeIter) -> Mode {
    for m in modes {
        if m.info().resolution() == (1920, 1080) {
            info!("Found good mode:\n{:#?}", m);
            return m;
        }
    }

    panic!("Couldn't find a good mode!");
}

pub fn read_file(path: &str) -> FileSystemResult<Vec<u8>> {
    let path = CString16::try_from(path).expect("Unable to convert path (&str) to CString16!");
    let fs = boot::get_image_file_system(boot::image_handle())
        .expect("Unable to get image file system!");
    let mut fs = FileSystem::new(fs);
    fs.read(path.as_ref())
}

#[entry]
fn main() -> Status {
    helpers::init().unwrap();

    let gop_handle =
        boot::get_handle_for_protocol::<GraphicsOutput>().expect("Unable to find the GOP!");
    let mut gop = boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle)
        .expect("Unable to get the GOP!");
    let mode = get_good_mode(gop.modes());

    gop.set_mode(&mode).expect("Unable to set GOP mode!");

    let mut buf = Buffer::new(&mut gop, 1920, 1080);
    buf
        .fill_buffer(BltPixel::new(0, 0, 0))
        .expect("Unable to fill screen!");

    let font_data = read_file("font.ttf").expect("Unable to read font file!");

    let f = Font::from_bytes(font_data.as_slice(), FontSettings::default())
        .expect("Unable to create font from bytes!");

    fonts::draw_string(
        f.clone(),
        "How much mush could a mushboom boom if a mushboom could boom mush?".to_string(),
        12.0,
        &mut buf,
        0,
        0,
    );
    fonts::draw_string(
        f.clone(),
        "A mushboom can boom as much mush as a mushboom if a mushboom could boom mush.".to_string(),
        12.0,
        &mut buf,
        0,
        14,
    );

    fonts::draw_string(
        f.clone(),
        "this is a tab: [\t]".to_string(),
        12.0,
        &mut buf,
        50,
        50,
    );

    fonts::draw_string(
        f.clone(),
        "this is a newline: [\n]".to_string(),
        12.0,
        &mut buf,
        65,
        65,
    );

    loop {}
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
