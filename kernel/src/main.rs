#![no_std]
#![no_main]
#![feature(core_intrinsics, abi_x86_interrupt, str_from_raw_parts)]
#![allow(
    unsafe_op_in_unsafe_fn,
    internal_features,
    clippy::needless_return,
    clippy::missing_safety_doc,
    clippy::empty_loop
)]

/* TODO:
 * A not shitty executable loader
 * VirtIO Drivers
 * IDE
 * Shutting down the system through ACPI
 * Different fonts
 * A better bootloader (Limine)
 * A C/C++ library (like glibc or musl)
 * Support external drivers
 * Possible Raspberry Pi port..?
 */

extern crate alloc;

pub mod acpi;
pub mod allocator;
pub mod apic;
pub mod commandline;
pub mod display;
pub mod drivers;
pub mod elf;
pub mod executor;
pub mod font;
pub mod gdt;
pub mod interrupts;
pub mod keyboard;
pub mod memory;
pub mod pci;
pub mod png;
pub mod serial;
pub mod sleep;
pub mod syscall;

use acpi::init_pcie_from_acpi;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::{BootInfo, entry_point};
use core::arch::asm;
use core::error;
use core::fmt::{Arguments, Write};
use display::{Framebuffer, TTY};
use elf::Process;
use keyboard::ScancodeStream;
use memory::BootInfoFrameAllocator;
use spin::Mutex;
use spinning_top::Spinlock;
use x86_64::{
    PhysAddr, VirtAddr,
};

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

pub static mut PMO: u64 = 0;
pub const HANDLER: acpi::Handler = acpi::Handler;

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

    println!("\x1b[0m{}", include_str!("../../assets/ascii_art.txt"));

    let pkg_ver = env!("CARGO_PKG_VERSION");
    info!(
        "Running Lemoncake version {}m{}. (c) 2025, SniverDaBest",
        pkg_ver.split(".").next().unwrap_or("?"),
        pkg_ver.split(".").nth(1).unwrap_or("?")
    );

    warning!("This is a hobby project. Don't expect it to be stable, secure, or even work.");

    let pmo = info
        .physical_memory_offset
        .into_option()
        .expect("No physical memory offset found!");
    unsafe {
        PMO = pmo;
    }

    info!("Physical memory offset: {:#X}", pmo);

    info!("Getting mapping & frame allocator...");
    let mut mapper = unsafe { memory::init(VirtAddr::new(PMO)) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&info.memory_regions) };

    info!("Initializing heap...");
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Unable to initialize heap!");

    let res = if let Some(fb) = crate::FRAMEBUFFER.lock().as_mut() {
        (fb.fb.info().width, fb.fb.info().height)
    } else {
        panic!("Couldn't get resolution!");
    };

    info!("Resolution: {}x{}", res.0, res.1);

    info!("Displaying logo...");
    png::draw_png(include_bytes!("../../assets/logo.png"), res.0 - 64, 0);

    info!("Getting ACPI information...");
    let tables = unsafe {
        ::acpi::AcpiTables::from_rsdp(
            HANDLER,
            info.rsdp_addr
                .into_option()
                .expect("Unable to get the RSDP address!") as usize,
        )
        .expect("Unable to get ACPI tables from the RSDP!")
    };

    info!("Setting up PCIe (MCFG)...");
    if let Err(e) = unsafe { init_pcie_from_acpi(&tables, &mut mapper, &mut frame_allocator) } {
        warning!(
            "PCIe ECAM init failed. Falling back to legacy CF8/CFC. Error: {}",
            e
        );
    } else {
        info!("PCIe ECAM initialized using MMIO ECAM.");
    }

    info!("Scanning PCIe bus...");
    let devs = pci::scan_pci_bus();

    info!("Initializing IDT & GDT...");
    interrupts::init_idt();
    gdt::init();

    let pi = tables
        .platform_info()
        .expect("Unable to get platform info!");

    info!("Setting up PICS/APIC");
    if let ::acpi::InterruptModel::Apic(apic) = pi.interrupt_model {
        let lapic = unsafe { apic::LocalApic::init(PhysAddr::new(apic.local_apic_address)) };
        let mut freq = 1_000_000;
        if let Some(cpuid) = apic::cpuid()
            && let Some(tsc) = cpuid.get_tsc_info()
        {
            freq = tsc.nominal_frequency();
        }

        unsafe {
            lapic.set_div_conf(0b1011);
            lapic.set_lvt_timer((1 << 17) + 48);
            lapic.set_init_count(freq);
        }

        for ioapic in apic.io_apics.iter() {
            let ioa = apic::IoApic::init(ioapic);
            for i in 0..24 {
                let n = ioa.read_redtlb(i);
                let mut red = apic::RedTbl::new(n);
                red.vector = (50 + i) as u8;
                let stored = red.store();
                ioa.write_redtlb(i, stored);
            }
        }
    } else {
        panic!("The legacy PICS are not supported!");
    }

    info!("Enabling interrupts...");
    x86_64::instructions::interrupts::enable();

    info!("Initializing scancode queue...");
    #[allow(unused)]
    let scancodes = Arc::new(Mutex::new(ScancodeStream::new()));

    info!("Finding IDE devices...");
    let mut ide_devs = Vec::new();
    for d in devs {
        if d.class_code == 0x1 && d.subclass == 0x1 {
            unsafe { d.write_config_u8(0x09, d.read_config_u8(0x09) | 0x05) };
            match d.prog_if() {
                0x5 | 0xF => ide_devs.push(d),

                0x85 | 0x8F => {
                    ide_devs.push(d);
                    unsafe {
                        d.enable_bus_master();
                    }
                }

                0x0 | 0xA => {
                    error!(
                        "Device is unsupported, due to it being ISA compat mode only! (no bus mastering)"
                    );
                }

                0x80 | 0x8A => {
                    error!(
                        "Device is unsupported, due to it being ISA compat mode only! (w/ bus mastering)"
                    );
                }

                u => {
                    warning!("Device has unknown prog if {:#x}! Assuming okay...", u);
                    ide_devs.push(d);
                }
            }
        }
    }

    info!("Found {} IDE devices!", ide_devs.len());

    if !ide_devs.is_empty() {
        info!("Initializing IDE devices...");
        for d in ide_devs {
            unsafe {
                drivers::ide::init_ide(d, &mut mapper, &mut frame_allocator);
            }
        }
    }

    info!("Switching to usermode...");
    unsafe { Process::new(include_bytes!("../../init")).switch(10, &mut mapper, &mut frame_allocator); };
    
    panic!("Error while switching to usermode!");
}

pub fn hlt_loop() -> ! {
    use x86_64::instructions::hlt;
    loop {
        hlt();
    }
}

pub unsafe fn rdrand() -> Option<u64> {
    let mut value: u64;
    let mut ok: u8;
    asm!(
        "rdrand {val}",
        "setc {okb}",
        val = out(reg) value,
        okb = out(reg_byte) ok,
        options(nostack, nomem)
    );
    if ok != 0 { Some(value) } else { None }
}

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
macro_rules! all_print {
    ($($arg:tt)*) => {
        $crate::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! all_println {
    () => ($crate::all_print!("\n"));
    ($fmt:expr) => ($crate::all_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::all_print!(
        concat!($fmt, "\n"), $($arg)*));
}

#[macro_export]
#[cfg(feature = "status-faces")]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::all_println!(
            "\x1b[34m(o_o) [INFO ]:\x1b[0m {}",
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
#[cfg(not(feature = "status-faces"))]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::all_println!(
            "\x1b[34m[INFO ]:\x1b[0m {}",
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
#[cfg(feature = "status-faces")]
macro_rules! warning {
    ($($arg:tt)*) => {
        $crate::all_println!(
            "\x1b[33m(0_0) [WARN ]:\x1b[0m {}",
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
#[cfg(not(feature = "status-faces"))]
macro_rules! warning {
    ($($arg:tt)*) => {
        $crate::all_println!(
            "\x1b[33m[WARN ]:\x1b[0m {}",
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
#[cfg(feature = "status-faces")]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::all_println!(
            "\x1b[31m(X_X) [ERROR]:\x1b[0m {}",
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
#[cfg(not(feature = "status-faces"))]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::all_println!(
            "\x1b[31m[ERROR]:\x1b[0m {}",
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
#[cfg(feature = "status-faces")]
macro_rules! nftodo {
    ($($arg:tt)*) => {
        $crate::all_println!(
            "\x1b[35m(-_-) [TODO ]:\x1b[0m {}",
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
#[cfg(not(feature = "status-faces"))]
macro_rules! nftodo {
    ($($arg:tt)*) => {
        $crate::all_println!(
            "\x1b[35m[TODO ]:\x1b[0m {}",
            format_args!($($arg)*)
        );
    };
}

#[macro_export]
macro_rules! sad {
    () => {
        if let Some(tty) = $crate::TTY.lock().as_mut() {
            tty.sad(Some((243, 139, 168, 255)));
        }
    };
}

#[macro_export]
macro_rules! yay {
    () => {
        if let Some(tty) = $crate::TTY.lock().as_mut() {
            tty.yay(Some((166, 227, 161, 255)));
        }
    };
}

/// Clear the TTY
#[macro_export]
macro_rules! clear {
    () => {{
        if let Some(tty) = $crate::TTY.lock().as_mut() {
            tty.clear_tty();
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

    #[cfg(feature = "serial-faces")]
    serial_print!("☹"); // this may not render in all terminals! disable the `serial-faces` feature to get rid of it.

    if let Some(tty) = TTY.lock().as_mut() {
        tty.sad(Some((243, 139, 168, 255)));
    }

    loop {}
}
