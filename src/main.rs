#![no_std]
#![no_main]
#![allow(clippy::needless_return, unsafe_op_in_unsafe_fn)]

extern crate alloc;

pub mod kernel_booting;
pub mod serial;
use alloc::vec::*;
use kernel_booting::*;
use uefi::{
    CString16,
    boot::{MemoryType, exit_boot_services},
    fs::*,
    helpers,
    prelude::*,
    proto::console::gop::{GraphicsOutput, Mode, ModeIter},
};

fn get_good_mode(modes: ModeIter) -> Mode {
    for m in modes {
        return if m.info().resolution() == (800, 600) {
            m
        } else {
            continue;
        };
    }

    panic!("Couldn't find good GOP mode!");
}

pub fn read_file(path: &str) -> FileSystemResult<Vec<u8>> {
    let path = CString16::try_from(path).expect("Unable to convert path (&str) to CString16!");
    let fs = boot::get_image_file_system(boot::image_handle())
        .expect("Unable to get image file system!");
    let mut fs = FileSystem::new(fs);
    fs.read(path.as_ref())
}

#[entry]
#[allow(mutable_transmutes, unreachable_code)]
fn main() -> Status {
    helpers::init().unwrap();

    serial_print!("Loading kernel...           ");
    let file = read_file("kernel").expect("Unable to get kernel file!");
    let kernel = file.as_slice();
    let data = KernelHeader::from_memory(kernel.as_ptr());
    let info = data.1;
    let header = data.0;
    serial_println!("...Done!");

    serial_print!("Getting kernel entry...     ");
    let entry_data = unsafe { header.get_entry(kernel) };
    let kmain = entry_data.0;
    let base_entry_addr = entry_data.1;
    let entry_addr = entry_data.2;
    serial_println!("...Done!");

    serial_print!("Getting GOP handle...       ");
    let gop_handle =
        boot::get_handle_for_protocol::<GraphicsOutput>().expect("Unable to find the GOP!");
    let mut gop = boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle)
        .expect("Unable to get the GOP!");
    let mode = get_good_mode(gop.modes());
    gop.set_mode(&mode).expect("Unable to set GOP mode!");

    let mut fb = gop.frame_buffer();
    let fb_addr = fb.as_mut_ptr();
    let res = gop.current_mode_info().resolution();
    serial_println!("...Done!");

    serial_print!("Exiting boot services...    ");
    let _mmap = unsafe { exit_boot_services(MemoryType::LOADER_DATA) };
    serial_println!("...Done!\n");

    if info != "" {
        serial_println!("{}", info);
    }
    serial_println!(
        "Entry Point located at {:X?} ({:X?})",
        base_entry_addr,
        entry_addr
    );
    serial_println!("Framebuffer address: {:?}", fb_addr);
    serial_println!("Framebuffer resolution: {}x{}\n", res.0, res.1);

    serial_println!("Launching kernel...\n");

    unsafe {
        kmain(fb_addr);
    }

    panic!("Kernel returned!?");
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
