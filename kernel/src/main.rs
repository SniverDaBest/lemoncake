#![no_std]
#![no_main]
#![feature(core_intrinsics, abi_x86_interrupt)]
#![allow(unsafe_op_in_unsafe_fn, internal_features, clippy::needless_return)]

pub static VERSION: &str = "25m4";

extern crate alloc;

pub mod allocator;
pub mod ahci;
pub mod display;
pub mod font;
pub mod fs;
pub mod gdt;
pub mod pci;
pub mod interrupts;
pub mod memory;
pub mod serial;

use alloc::vec::Vec;
#[allow(unused_imports)]
use ansi_rgb::{red, yellow, Foreground, WithForeground};

use bootloader_api::{entry_point, BootInfo};
use bootloader_api::config::{BootloaderConfig, Mapping};
use display::Framebuffer;
use memory::BootInfoFrameAllocator;
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

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::serial_println!(
            "{} {}",
            $crate::colorize("(o_o) [INFO]:", ansi_rgb::blue()),
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
    };
}

macro_rules! success {
    ($($arg:tt)*) => {
        $crate::serial_println!(
            "{} {}",
            $crate::colorize("(^_^) [SUCCESS]:", ansi_rgb::green()),
            format_args!($($arg)*)
        );
    };
}

fn kernel_main(info: &'static mut BootInfo) -> ! {
    let ver = env!("CARGO_PKG_VERSION").split('.');
    serial_print!("{} Running Lemoncake version ", colorize("(o_o) [INFO]:", ansi_rgb::blue()));
    for x in ver {
        serial_print!("{}.", x);
    }
    serial_println!();

    warning!("This is a hobby project. Don't expect it to be stable, secure, or even work.");
    
    let blfb = info.framebuffer.take().expect("No framebuffer found!");
    let mut fb = Framebuffer::new(blfb);
    fb.clear_screen((0,0,0));
    fb.put_pixel(1,0,(255,255,255));
    fb.put_pixel(1,1,(255,255,255));
    
    fb.put_pixel(5,0,(255,255,255));
    fb.put_pixel(5,1,(255,255,255));
    
    fb.put_pixel(0,4,(255,255,255));
    fb.put_pixel(1,5,(255,255,255));
    fb.put_pixel(2,5,(255,255,255));
    fb.put_pixel(3,5,(255,255,255));
    fb.put_pixel(4,5,(255,255,255));
    fb.put_pixel(5,5,(255,255,255));
    fb.put_pixel(6,4,(255,255,255));
    
    info!("Initializing GDT, IDT, PICS, and enabling interrupts...");
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize(); }
    x86_64::instructions::interrupts::enable();
    
    info!("Initializing Heap...");
    let pmo = info.physical_memory_offset.into_option().expect("No physical memory offset found!");
    let mut mapper = unsafe { memory::init(VirtAddr::new(pmo)) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&info.memory_regions) };
    
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Unable to initialize heap!");
    
    info!("Looking for AHCI devices...");
    let ahci_devices = unsafe { ahci::find_ahci_devices(&mut mapper, &mut frame_allocator) };
    if ahci_devices.is_empty() {
        info!("No AHCI devices found.");
    } else {
        for mut device in ahci_devices {
                info!("Found AHCI controller at {}:{}.{}", 
                device.pci_device.bus, 
                device.pci_device.device_id, 
                device.pci_device.func
            );
            
            for port in &device.ports {
                if port.is_implemented {
                    info!("  Port {}: Type {}", port.port_number, port.port_type);
                }
            }
            device.init(&mut mapper, &mut frame_allocator).expect("Unable to initialize AHCI device!");
            //ahci::test_ahci_read_write(&mut device);
        }
    }

    success!("Done setting up!");
    info!("TODO:\n- Font Rendering\n- Image Rendering\n- ACPI");

    fb.put_pixel(70,69,(0,255,0));
    fb.put_pixel(71,69,(0,255,0));
    fb.put_pixel(72,69,(0,255,0));
    fb.put_pixel(69,70,(0,255,0));
    fb.put_pixel(69,71,(0,255,0));
    fb.put_pixel(69,72,(0,255,0));
    fb.put_pixel(69,73,(0,255,0));
    fb.put_pixel(69,74,(0,255,0));
    fb.put_pixel(70,75,(0,255,0));
    fb.put_pixel(71,75,(0,255,0));
    fb.put_pixel(72,75,(0,255,0));
    fb.put_pixel(73,74,(0,255,0));
    fb.put_pixel(73,73,(0,255,0));
    fb.put_pixel(73,72,(0,255,0));
    fb.put_pixel(73,71,(0,255,0));
    fb.put_pixel(73,70,(0,255,0));

    fb.put_pixel(75,69,(0,255,0));
    fb.put_pixel(75,70,(0,255,0));
    fb.put_pixel(75,71,(0,255,0));
    fb.put_pixel(75,72,(0,255,0));
    fb.put_pixel(75,73,(0,255,0));
    fb.put_pixel(75,74,(0,255,0));
    fb.put_pixel(75,75,(0,255,0));
    fb.put_pixel(76,73,(0,255,0));
    fb.put_pixel(77,72,(0,255,0));
    fb.put_pixel(77,71,(0,255,0));
    fb.put_pixel(77,70,(0,255,0));
    fb.put_pixel(77,69,(0,255,0));
    fb.put_pixel(77,74,(0,255,0));
    fb.put_pixel(77,75,(0,255,0));

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
    error!("\nUh-oh! The Lemoncake kernel needed to panic.\nHere's what happened:\nPanic Message: {}\nLocation: {}@L{}:{}",
        _info.message().fg(red()),
        _info.location().unwrap().file().fg(yellow()),
        _info.location().unwrap().line(),
        _info.location().unwrap().column());
    
    loop {}
}