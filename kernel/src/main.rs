#![no_std]
#![no_main]
#![feature(core_intrinsics, abi_x86_interrupt, str_from_raw_parts)]
#![allow(unsafe_op_in_unsafe_fn, internal_features, clippy::needless_return)]

/* TODO:
 * Scroll down when TTY cursor y pos >= TTY height
 * Shutting down the system through ACPI
 * Working usermode
 * A C/C++ library (like glibc or musl)
 * Support external drivers
 */

extern crate alloc;

pub mod acpi;
pub mod allocator;
pub mod apic;
pub mod commandline;
pub mod disks;
pub mod display;
pub mod elf;
pub mod executor;
pub mod font;
pub mod fs;
pub mod gdt;
pub mod interrupts;
pub mod keyboard;
pub mod memory;
pub mod pci;
pub mod png;
pub mod serial;
pub mod sleep;
pub mod syscall;

use alloc::sync::Arc;
use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::{BootInfo, entry_point};
use commandline::run_command_line;
use core::error;
use core::fmt::{Arguments, Write};
use display::{Framebuffer, TTY};
use executor::{Executor, Task};
use keyboard::ScancodeStream;
use memory::BootInfoFrameAllocator;
use spin::Mutex;
use spinning_top::Spinlock;
use x86_64::registers::control::{Efer, EferFlags};
use x86_64::registers::model_specific::Msr;
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

    info!("Displaying logo...");
    png::draw_png(include_bytes!("../../assets/logo.png"), 1206, 0);

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

    info!("Initializing IDT & GDT...");
    interrupts::init_idt();
    gdt::init();

    let pi = tables
        .platform_info()
        .expect("Unable to get platform info!");

    info!("Setting up PICS/APIC");
    if let ::acpi::InterruptModel::Apic(apic) = pi.interrupt_model {
        info!("Using APIC!");
        let lapic = unsafe { apic::LocalApic::init(PhysAddr::new(apic.local_apic_address)) };
        let mut freq = 1000_000;
        if let Some(cpuid) = apic::cpuid() {
            if let Some(tsc) = cpuid.get_tsc_info() {
                freq = tsc.nominal_frequency();
            }
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
    let scancodes = Arc::new(Mutex::new(ScancodeStream::new()));

    /*info!("Looking for AHCI devices...");
    let devices = disks::ahci::scan_for_ahci_controllers();
    let mut ahci_devs: Vec<AHCIController> = Vec::new();

    for dev in devices {
        info!("PCI Device:\n{:#?}", dev);
        let c = unsafe { AHCIController::from_pci(dev, &mut mapper, &mut frame_allocator) };
        if c.is_some() {
            ahci_devs.push(c.unwrap());
        }
    }

    info!("Found {} AHCI devices in total.", ahci_devs.len());

    info!("Scanning for SFS filesystems...");
    for dev in ahci_devs.iter_mut() {
        info!("Ports: {:?}", dev.ports);
        if let Some(mut sfs) = fs::sfs::SFS::probe_on_device(dev) {
            match sfs.mount() {
                Ok(_) => {
                    info!("Mounted SFS filesystem on device!");
                }
                Err(e) => {
                    error!("Failed to mount SFS! Error: {:?}", e);
                }
            }
        } else {
            info!("No SFS filesystem found on device");
        }
    }

    info!("Setting up syscalls...");
    unsafe {
        syscall::init_syscalls();
    }

    info!("Loading init program...");
    let e = load_elf(
        include_bytes!("../../init"),
        &mut mapper,
        &mut frame_allocator,
    )
    .expect("Unable to get init program address!");

    info!("Mapping the user stack...");
    map_user_stack(&mut mapper, &mut frame_allocator);

    info!("Switching to usermode at {:#x}...", e);
    unsafe {
        switch_to_user(e.as_u64(), 0x7FFF_FFFF_F000);
    }*/

    info!("Checking for NVME devices...");
    unsafe {
        disks::nvme::nvme_init(&mut mapper, &mut frame_allocator);
    }

    let mut e = Executor::new();
    e.spawn(Task::new(run_command_line(scancodes)));
    e.run();
}

pub fn map_user_stack(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    let user_stack_start = VirtAddr::new(0x7FFF_FFFF_D000);
    let user_stack_end = VirtAddr::new(0x7FFF_FFFF_E000);

    info!("User stack range:");
    info!("{:?} - {:?}", user_stack_start, user_stack_end);

    let page_range = Page::range_inclusive(
        Page::containing_address(user_stack_start),
        Page::containing_address(user_stack_end),
    );

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .expect("Unable to allocate frame for user stack!");
        unsafe {
            mapper
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::USER_ACCESSIBLE,
                    frame_allocator,
                )
                .expect("Unable to map the user stack!")
                .flush();
        }
    }
}

pub unsafe fn switch_to_user(entry: u64, user_stack_top: u64) {
    Efer::update(|e| *e |= EferFlags::SYSTEM_CALL_EXTENSIONS);
    let star = ((0x1Bu64) << 48) | ((0x08u64) << 32);
    Msr::new(0xC000_0081).write(star);
    Msr::new(0xC000_0082).write(entry);
    Msr::new(0xC000_0084).write(0x0000_0000_0000_0001);
    syscall::jump_to_usermode(entry, user_stack_top);
}

pub fn hlt_loop() -> ! {
    use x86_64::instructions::hlt;
    loop {
        hlt();
    }
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
