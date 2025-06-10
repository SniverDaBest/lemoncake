#![no_std]
#![no_main]
#![feature(core_intrinsics, abi_x86_interrupt)]
#![allow(unsafe_op_in_unsafe_fn, internal_features, clippy::needless_return)]

/* TODO:
 * Fix APIC/PICS
 * Shutting down the system (w/o force closing Qemu, VBox, etc.)
 * Usermode
 * Support running apps (in usermode)
 * A C/C++ library (like glibc or musl)
 * Support external drivers
 */

extern crate alloc;

pub mod ahci;
pub mod allocator;
pub mod commandline;
pub mod display;
pub mod executor;
pub mod font;
pub mod fs;
pub mod gdt;
pub mod interrupts;
pub mod keyboard;
pub mod memory;
pub mod pci;
pub mod serial;

use crate::memory::BootInfoFrameAllocator;
use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::{BootInfo, entry_point};
use commandline::run_command_line;
use core::error;
use core::fmt::{Arguments, Write};
use display::{Framebuffer, TTY};
use executor::{Executor, Task};
use interrupts::setup_pics;
use spinning_top::Spinlock;
use x86_64::{
    VirtAddr,
    structures::paging::{Mapper, Page},
};

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

pub static FRAMEBUFFER: Spinlock<Option<Framebuffer>> = Spinlock::new(None);
pub static TTY: Spinlock<Option<TTY>> = Spinlock::new(None);

fn kernel_main(info: &'static mut BootInfo) -> ! {
    serial_print!("\x1B[2J\x1B[1;1H");

    let blfb = info.framebuffer.take().expect("No framebuffer found!");
    let fb = Framebuffer::new(blfb);
    *FRAMEBUFFER.lock() = Some(fb);
    *TTY.lock() = Some(TTY::new());

    if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
        fb.clear_screen((30, 30, 46));
    }

    let pkg_ver = env!("CARGO_PKG_VERSION");
    info!(
        "Running Lemoncake version {}m{}",
        pkg_ver.split(".").nth(0).unwrap_or("?"),
        pkg_ver.split(".").nth(1).unwrap_or("?")
    );

    warning!("This is a hobby project. Don't expect it to be stable, secure, or even work.");

    info!("Getting mapping & frame allocator...");
    let pmo = info
        .physical_memory_offset
        .into_option()
        .expect("No physical memory offset found!");
    let mut mapper = unsafe { memory::init(VirtAddr::new(pmo)) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&info.memory_regions) };

    info!("Initializing GDT...");
    gdt::init();

    info!("Verifying TSS stack mapping...");
    let (stack_start, stack_end) = gdt::timer_stack_bounds();
    for addr in (stack_start.as_u64()..stack_end.as_u64()).step_by(0x1000) {
        let page: Page = Page::containing_address(VirtAddr::new(addr));
        if mapper.translate_page(page).is_err() {
            panic!("TSS stack page at {:#X} is not mapped!", addr);
        }
    }

    info!("Initializing IDT and setting up PICS/APIC...");
    interrupts::init_idt();
    unsafe {
        setup_pics(&mut mapper, &mut frame_allocator);
    }

    info!("Initializing heap...");
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Unable to initialize heap!");

    info!("Enabling interrupts...");
    x86_64::instructions::interrupts::enable();

    info!("Looking for AHCI devices & SFS filesystems...");
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

    let mut e = Executor::new();
    e.spawn(Task::new(run_command_line()));
    e.run();
}

pub fn hlt_loop() -> ! {
    use x86_64::instructions::hlt;
    loop {
        hlt();
    }
}

#[macro_export]
macro_rules! tty_print {
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
macro_rules! tty_println {
    () => ($crate::tty_print!("\n"));
    ($fmt:expr) => ($crate::tty_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::tty_print!(
        concat!($fmt, "\n"), $($arg)*));
}

#[doc(hidden)]
pub fn _print(args: Arguments) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        serial::SERIAL1
            .lock()
            .write_fmt(args)
            .expect("Printing to serial failed");
    });
    if let Some(t) = TTY.lock().as_mut() {
        let _ = write!(t, "{}", args);
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::_print(format_args!($($arg)*))
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
        #[cfg(feature = "status-faces")]
        $crate::print!("\x1b[34m(o_o) ");
        $crate::println!(
            "\x1b[34m[INFO]:\x1b[0m {}",
            format_args!($($arg)*)
        );
    };
}

#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => {
        #[cfg(feature = "status-faces")]
        $crate::print!("\x1b[33m(0_0) ");
        $crate::println!(
            "\x1b[33m[WARNING]:\x1b[0m {}",
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        #[cfg(feature = "status-faces")]
        $crate::print!("\x1b[31m(X_X) ");
        $crate::println!(
            "\x1b[31m[ERROR]:\x1b[0m {}",
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => {
        #[cfg(feature = "status-faces")]
        $crate::print!("\x1b[32m(^_^) ");
        $crate::println!(
            "\x1b[32m[SUCCESS]:\x1b[0m {}",
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! nftodo {
    ($($arg:tt)*) => {
        #[cfg(feature = "status-faces")]
        $crate::print!("\x1b[35m(-_-) ");
        $crate::println!(
            "\x1b[35m[TODO]:\x1b[0m {}",
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! sad {
    ($($arg:tt)*) => {
        if let Some(tty) = $crate::TTY.lock().as_mut() {
            tty.sad(Some((243, 139, 168)));
        }
        #[cfg(feature = "serial-faces")]
        $crate::serial_print!("☹"); // this may not render in all terminals! disable the `serial-faces` feature to get rid of it.
    };
}

#[macro_export]
macro_rules! yay {
    ($($arg:tt)*) => {
        if let Some(tty) = $crate::TTY.lock().as_mut() {
            tty.yay(Some((166, 227, 161)));
        }
        #[cfg(feature = "serial-faces")]
        $crate::serial_print!("☺"); // this may not render in all terminals! disable the `serial-faces` feature to get rid of it.
    };
}

/// Clear the TTY
#[macro_export]
macro_rules! clear {
    () => {{
        $crate::serial_print!("\x1B[2J\x1B[1;1H");
        if let Some(fb) = $crate::FRAMEBUFFER.lock().as_mut() {
            fb.clear_screen((30, 30, 46));
        }
    }};
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    error!(
        "\nUh-oh! The Lemoncake kernel needed to panic.\nHere's what happened:\nPanic Message: \x1b[31m{}\x1b[0m\nLocation: \x1b[33m{}@L{}:{}\x1b[0m",
        _info.message(),
        _info.location().unwrap().file(),
        _info.location().unwrap().line(),
        _info.location().unwrap().column()
    );

    sad!();

    loop {}
}
