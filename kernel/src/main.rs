#![no_std]
#![no_main]
#![feature(core_intrinsics, abi_x86_interrupt)]
#![allow(unsafe_op_in_unsafe_fn, internal_features, clippy::needless_return)]

pub static VERSION: &str = "25m4";

extern crate alloc;

pub mod ahci;
pub mod allocator;
pub mod display;
pub mod font;
pub mod fs;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod pci;
pub mod serial;

#[allow(unused_imports)]
use ansi_rgb::{Foreground, WithForeground, red, yellow};

use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::{BootInfo, entry_point};
use display::{Framebuffer, TTY};
use memory::BootInfoFrameAllocator;
use spinning_top::Spinlock;
use x86_64::VirtAddr;

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

pub fn colorize(text: &str, color: rgb::Rgb<u8>) -> WithForeground<&str> {
    return text.fg(color);
}

pub static FRAMEBUFFER: Spinlock<Option<Framebuffer>> = Spinlock::new(None);
pub static TTY: Spinlock<Option<TTY>> = Spinlock::new(None);

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        if let Some(t) = $crate::TTY.lock().as_mut() {
            use core::fmt::Write;
            let _ = write!(t, "{}", format_args!($($arg)*));
        } else {
            $crate::serial_println!("No TTY available!");
        }
    };
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(
        concat!($fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::serial_println!(
            "{} {}",
            $crate::colorize("(o_o) [INFO]:", ansi_rgb::blue()),
            format_args!($($arg)*)
        );
        $crate::println!(
            "\x1b[34m(o_o) [INFO]:\x1b[0m {}",
            format_args!($($arg)*)
        );
    };
}

#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => {
        $crate::serial_println!(
            "{} {}",
            $crate::colorize("(0_0) [WARNING]:", ansi_rgb::yellow()),
            format_args!($($arg)*)
        );
        $crate::println!(
            "\x1b[33m(0_0) [WARNING]:\x1b[0m {}",
            format_args!($($arg)*)
        );
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::serial_println!(
            "{} {}",
            $crate::colorize("(X_X) [ERROR]:", ansi_rgb::red()),
            format_args!($($arg)*)
        );
        $crate::println!(
            "\x1b[31m(X_X) [ERROR]:\x1b[0m {}",
            format_args!($($arg)*)
        );
    };
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => {
        $crate::serial_println!(
            "{} {}",
            $crate::colorize("(^_^) [SUCCESS]:", ansi_rgb::green()),
            format_args!($($arg)*)
        );
        $crate::println!(
            "\x1b[32m(^_^) [SUCCESS]:\x1b[0m {}",
            format_args!($($arg)*)
        );
    };
}

fn kernel_main(info: &'static mut BootInfo) -> ! {
    serial_print!("\x1B[2J\x1B[1;1H");

    let blfb = info.framebuffer.take().expect("No framebuffer found!");
    let fb = Framebuffer::new(blfb);
    *FRAMEBUFFER.lock() = Some(fb);
    *TTY.lock() = Some(TTY::new());

    if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
        fb.clear_screen((30,30,46));
    }

    let pkg_ver = env!("CARGO_PKG_VERSION");
    info!(
        "Running Lemoncake version {}",
        pkg_ver
    );

    warning!("This is a hobby project. Don't expect it to be stable, secure, or even work.");

    info!("Initializing GDT, IDT, PICS, and enabling interrupts...");
    gdt::init();
    interrupts::init_idt();
    unsafe {
        interrupts::PICS.lock().initialize();
    }
    x86_64::instructions::interrupts::enable();

    info!("Initializing Heap...");
    let pmo = info
        .physical_memory_offset
        .into_option()
        .expect("No physical memory offset found!");
    let mut mapper = unsafe { memory::init(VirtAddr::new(pmo)) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&info.memory_regions) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Unable to initialize heap!");

    info!("Looking for AHCI devices...");
    let ahci_devices = unsafe { ahci::find_ahci_devices(&mut mapper, &mut frame_allocator) };
    if ahci_devices.is_empty() {
        info!("No AHCI devices found.");
    } else {
        for mut device in ahci_devices {
            info!(
                "Found AHCI controller at {}:{}.{}",
                device.pci_device.bus, device.pci_device.device_id, device.pci_device.func
            );

            for port in &device.ports {
                if port.is_implemented {
                    info!("  Port {}: Type {}", port.port_number, port.port_type);
                }
            }
            device
                .init(&mut mapper, &mut frame_allocator)
                .expect("Unable to initialize AHCI device!");
        }
    }

    success!("Done setting up!");
    info!("TODO:\n- Font Rendering\n- Image Rendering\n- ACPI");

    loop {}
}

pub fn hlt_loop() -> ! {
    use x86_64::instructions::hlt;
    loop {
        hlt();
    }
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    error!(
        "\nUh-oh! The Lemoncake kernel needed to panic.\nHere's what happened:\nPanic Message: {}\nLocation: {}@L{}:{}",
        _info.message().fg(red()),
        _info.location().unwrap().file().fg(yellow()),
        _info.location().unwrap().line(),
        _info.location().unwrap().column()
    );

    if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
        fb.draw_sad_face(100, 0, (255, 0, 0));
    }

    loop {}
}
