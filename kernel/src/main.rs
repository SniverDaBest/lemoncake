#![no_std]
#![no_main]
#![allow(unsafe_op_in_unsafe_fn)]

pub mod serial;

//#[derive(Debug, Clone, Copy)]
//#[repr(C, packed)]
//pub struct KernelHeader {
//    /// Should **ALWAYS** be equal to b"kb00t!"
//    pub magic: [u8; 6],
//    /// Kernel header revision number
//    pub revision: u16,
//    /// The entry point function
//    pub entry_offset: u64,
//}
//
//impl KernelHeader {
//    pub const fn new(addr: u64) -> Self {
//        KernelHeader {
//            magic: *b"kb00t!",
//            revision: 1,
//            entry_offset: addr,
//        }
//    }
//}
//
//#[used]
//#[unsafe(link_section = ".kboot_hdr")]
//#[unsafe(no_mangle)]
//pub static HEADER: KernelHeader = KernelHeader::new(core::mem::transmute::<unsafe extern "C" fn(*mut u8) -> !, u64>(kmain));

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain(fb_addr: *mut u8) -> ! {
    serial_println!("Loaded kernel! :D");
    serial_println!("Framebuffer addr: {:?}", fb_addr);

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
