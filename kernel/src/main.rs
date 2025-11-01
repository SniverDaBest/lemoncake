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
 * Fix the disaster at display.rs@L224
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
use alloc::{sync::Arc, boxed::Box};
use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::{BootInfo, entry_point};
use commandline::run_command_line;
use core::arch::asm;
use core::error;
use core::fmt::{Arguments, Write};
use display::{Framebuffer, TTY};
use drivers::ustar::USTar;
use elf::Process;
use executor::*;
use keyboard::ScancodeStream;
use memory::BootInfoFrameAllocator;
use spin::Mutex;
use spinning_top::Spinlock;
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

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

pub static FS: Spinlock<Option<&'static mut USTar>> = Spinlock::new(None);

fn kernel_main(info: &'static mut BootInfo) -> ! {
    serial_print!("\x1B[2J\x1B[1;1H");

    let blfb = info.framebuffer.take().expect("No framebuffer found!");
    let fb = Framebuffer::new(blfb);
    *FRAMEBUFFER.lock() = Some(fb);
    *TTY.lock() = Some(TTY::new());

    if let Some(tty) = TTY.lock().as_mut() {
        tty.clear_tty();
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
    let _devs = pci::scan_pci_bus();

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

    /*
    info!("Initializing NVMe devices...");
    unsafe {
        match nvme::nvme_init(&mut mapper, &mut frame_allocator) {
            Ok(_) => {}
            Err(e) => {
                error!("Failed initializing NVMe devices! Error: {:?}", e);
            }
        }
    }
    */

    info!("Initializing ramdisk...");
    let ramdisk_data = include_bytes!("../../hd.tar");
    *FS.lock() = unsafe { init_ramdisk(&mut mapper, &mut frame_allocator, 50, ramdisk_data) };

    info!("Getting init program...");
    let init = if let Some(fs) = FS.lock().as_ref() {
        let i = fs.read_file("init".as_bytes());
        if i.is_none() { error!("Unable to read init program! Going to kernel shell..."); }
        i
    } else {
        error!("Unable to get ramdisk! Going to kernel shell...");
        None
    };
    
    match init {
        Some(i) => {
            info!("Switching to usermode...");
            serial_println!("Don't expect much more output here!");
            unsafe {
                panic!(
                    "Error while switching to usermode! Error: {:?}",
                    Process::new(i.read_all()).switch(10, &mut mapper, &mut frame_allocator)
                );
            };
        }
        
        None => {
            let mut e = Executor::new();
            e.spawn(Task::new(run_command_line(scancodes)));
            e.run();
        }
    }
}

unsafe fn init_ramdisk<M: Mapper<Size4KiB>, F: FrameAllocator<Size4KiB>>(
    mapper: &mut M,
    frame_allocator: &mut F,
    mib: usize,
    ramdisk_data: &'static [u8],
) -> Option<&'static mut USTar> {
    let start = VirtAddr::new(0x5000_0000_0000);
    let end = start + mib as u64 * 1024 * 1024;
    let start_page = Page::containing_address(start);
    let end_page = Page::containing_address(end);
    for page in Page::range_inclusive(start_page, end_page) {
        let frame = frame_allocator
            .allocate_frame()
            .expect("Unable to allocate a frame!");

        mapper
            .map_to(
                page,
                frame,
                PageTableFlags::WRITABLE | PageTableFlags::PRESENT,
                frame_allocator,
            )
            .expect("Unable to map a page for the ramdisk!")
            .flush();
    }

    let rd = core::slice::from_raw_parts_mut(start.as_mut_ptr::<u8>(), ramdisk_data.len());
    rd.copy_from_slice(ramdisk_data);

    let ustar = drivers::ustar::USTar::new(rd);
    return Some(Box::leak(Box::new(ustar)));
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
        )
    };
}

#[macro_export]
macro_rules! sad {
    () => {
        if let Some(tty) = $crate::TTY.lock().as_mut() {
            tty.sad(Some(crate::display::RED));
        }
    };
}

#[macro_export]
macro_rules! yay {
    () => {
        if let Some(tty) = $crate::TTY.lock().as_mut() {
            tty.yay(Some(crate::display::GREEN));
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
        tty.sad(Some(crate::display::RED));
    }

    loop {}
}
