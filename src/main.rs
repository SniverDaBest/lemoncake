#![no_std]
#![no_main]
#![allow(
    clippy::needless_return,
)]

pub mod serial;
use uefi::{
    boot::{exit_boot_services, MemoryType}, helpers, prelude::*, proto::console::gop::{GraphicsOutput, Mode, ModeIter},
};

fn get_good_mode(modes: ModeIter) -> Mode {
    for m in modes {
        if m.info().resolution() == (640, 480) {
            serial_println!("Found good mode:\n{:#?}", m);
            return m;
        }
    }

    panic!("Couldn't find a good mode!");
}

#[entry]
#[allow(mutable_transmutes)]
fn main() -> Status {
    helpers::init().unwrap();

    let gop_handle =
        boot::get_handle_for_protocol::<GraphicsOutput>().expect("Unable to find the GOP!");
    let mut gop = boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle)
        .expect("Unable to get the GOP!");
    let mode = get_good_mode(gop.modes());
    gop.set_mode(&mode).expect("Unable to set GOP mode!");

    let mut fb = gop.frame_buffer();
    let fb_addr = fb.as_mut_ptr();
    let res = gop.current_mode_info().resolution();

    serial_println!("%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%\nFramebuffer info:\nAddress: {:?}\nResolution: {}x{}\n%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%", fb_addr, res.0, res.1);
    serial_println!("Booting Lemoncake...");

    let _mmap = unsafe { exit_boot_services(MemoryType::LOADER_DATA) };

    panic!("uhh i haven't implemented this yet.");
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    serial_println!(
        "(X_X)\n\nUh-Oh, Lemoncake panicked!\nMessage: {}\nLocation: {}@L{}:{}",
        info.message(),
        info.location().unwrap().file(),
        info.location().unwrap().line(),
        info.location().unwrap().column()
    );

    loop {}
}