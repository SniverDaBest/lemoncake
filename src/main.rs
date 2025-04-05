#![no_std]
#![no_main]
#![feature(alloc_error_handler, abi_x86_interrupt, core_intrinsics)]
#![allow(
    static_mut_refs,
    clippy::needless_return,
    unsafe_op_in_unsafe_fn,
    clippy::missing_safety_doc
)]

extern crate alloc;

pub mod display;
pub mod fonts;

use alloc::vec::*;
use display::{Buffer, TTY, TTYColors};
use fontdue::{Font, FontSettings};
use log::{error, info};
use spinning_top::Spinlock;
use uefi::{
    CString16,
    fs::{FileSystem, FileSystemResult},
    helpers,
    prelude::*,
    proto::console::gop::{BltPixel, GraphicsOutput, Mode, ModeIter},
};

pub const LEMONCAKE_VER: &str = "25m3-UEFI";
pub static TERM: Spinlock<TTY> = Spinlock::new(TTY::new());

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
    buf.fill_buffer(BltPixel::new(0, 0, 0))
        .expect("Unable to clean the screen!");

    let font_data = read_file("font.ttf").expect("Unable to read font file!");

    let f = Font::from_bytes(font_data.as_slice(), FontSettings::default())
        .expect("Unable to create font from bytes!");

    TERM.lock().set_colors(TTYColors::default());
    TERM.lock().write_str(&mut buf, &f, "god this text is mangled lol\nabcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890`~!@#$%^&*()-_=+[{}]\\|;:'\",<.>/?").expect("Unable to write text to TTY!");

    info!("Done!");
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
