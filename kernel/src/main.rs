#![no_std]
#![no_main]

pub mod serial;

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain(fb_addr: *mut u8) -> ! {
    serial_println!("Loaded kernel! :D");
    serial_println!("Framebuffer addr: {:?}", fb_addr);

    panic!("Finished.");
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
