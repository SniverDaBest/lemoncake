#![no_std]
#![no_main]
#![allow(unsafe_op_in_unsafe_fn)]

pub mod serial;

unsafe extern "C" {
    static _kmain_addr: u8;
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct KernelHeader {
    /// Should **ALWAYS** be equal to b"kb00t!"
    pub magic: [u8; 6],
    /// Kernel header revision number
    pub revision: u16,
    /// The entry point function
    pub entry_offset: u64,
}

#[used]
#[unsafe(link_section = ".kboot_hdr")]
#[unsafe(no_mangle)]
pub static mut HEADER: KernelHeader = KernelHeader {
    magic: *b"kb00t!",
    revision: 1,
    entry_offset: 0,
};

#[unsafe(no_mangle)]
fn kmain(fb_addr: *mut u8) -> ! {
    serial_println!("Loaded kernel! :D");
    serial_println!("Framebuffer addr: {:?}", fb_addr);
    
    unsafe {
        let kernel_base = &_kmain_addr as *const u8 as u64;
        let kmain_addr = kmain as *const () as u64;
        HEADER.entry_offset = kmain_addr - kernel_base;
    }

    loop {}
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
