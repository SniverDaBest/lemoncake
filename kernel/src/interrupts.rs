use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

use crate::{error, gdt, hlt_loop, info, sad, serial_print};
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spinning_top::Spinlock;
use x86_64::PhysAddr;
use x86_64::VirtAddr;
use x86_64::registers::model_specific::Msr;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use x86_64::structures::paging::PageTableFlags;
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PhysFrame, Size4KiB};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
pub const USING_APIC: Spinlock<bool> = Spinlock::new(true);
const IA32_APIC_BASE_MSR: u32 = 0x1B;
const IA32_APIC_BASE_MSR_ENABLE: u32 = 0x800;
const APIC_VIRT_BASE: u64 = 0xFFFF_FF00_0000_0000;
const APIC_SVR_OFFSET: usize = 0xF0;
const APIC_LVT_TIMER_OFFSET: usize = 0x320;
const APIC_TIMER_INITCNT_OFFSET: usize = 0x380;
const APIC_TIMER_DIV_OFFSET: usize = 0x3E0;
const APIC_TIMER_PERIODIC: u32 = 0x20000;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }
}

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);

        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);

            idt[InterruptIndex::Timer.as_u8()]
                .set_handler_fn(timer_interrupt_handler)
                .set_stack_index(1);
        }

        idt
    };
}

pub unsafe fn disable_pics() {
    PICS.lock().initialize();
    PICS.lock().write_masks(0xFF, 0xFF);
}

unsafe fn check_apic() -> bool {
    let mut edx: u32;

    asm!(
        "cpuid",
        inout("eax") 1 => _,
        lateout("edx") edx,
    );

    return (edx & (1 << 9)) != 0;
}

unsafe fn set_apic_base(apic: usize) {
    let edx: u32 = (apic as u64 >> 32) as u32;
    let eax: u32 = ((apic & 0xFFFFF000) | IA32_APIC_BASE_MSR_ENABLE as usize) as u32;
    let value: u64 = ((edx as u64) << 32) | (eax as u64);

    let mut msr = Msr::new(IA32_APIC_BASE_MSR);
    msr.write(value);
}

unsafe fn get_apic_base() -> usize {
    return APIC_VIRT_BASE as usize;
}

unsafe fn get_apic_phys_base() -> usize {
    let msr = Msr::new(IA32_APIC_BASE_MSR);
    let value = msr.read();
    let eax: u32 = value as u32;
    let edx: u32 = (value >> 32) as u32;

    return (eax as usize & 0xfffff000) | ((edx as usize & 0x0f) << 32);
}

unsafe fn read_reg(offset: usize) -> u32 {
    let apic_base = get_apic_base();
    let reg_ptr = (apic_base + offset) as *const u32;
    return read_volatile(reg_ptr);
}

unsafe fn write_reg(offset: usize, value: u32) {
    let apic_base = get_apic_base();
    let reg_ptr = (apic_base + offset) as *mut u32;
    write_volatile(reg_ptr, value);
}

unsafe fn apic_eoi() {
    write_reg(0xB0, 0);
}

pub unsafe fn setup_pics(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    PICS.lock().initialize();

    if check_apic() {
        info!("(APIC) Using the APIC!");
        disable_pics();
        init_pit(100);

        let apic_phys_base = get_apic_phys_base();
        let apic_start_frame = PhysFrame::containing_address(PhysAddr::new(apic_phys_base as u64));
        let apic_end_frame =
            PhysFrame::containing_address(PhysAddr::new((apic_phys_base + 0xFFF) as u64));
        let apic_start_page = Page::containing_address(VirtAddr::new(APIC_VIRT_BASE));
        let apic_end_page = Page::containing_address(VirtAddr::new(APIC_VIRT_BASE + 0xFFF));
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE;

        for (page, frame) in Page::range_inclusive(apic_start_page, apic_end_page)
            .zip(PhysFrame::range_inclusive(apic_start_frame, apic_end_frame))
        {
            info!(
                "(APIC) Mapping page: {:?} with flags {:?} on frame {:?}",
                page, flags, frame
            );
            mapper
                .map_to(page, frame, flags, frame_allocator)
                .expect("(APIC) Unable to map the APIC memory region!")
                .flush();
        }

        set_apic_base(apic_phys_base);

        write_reg(0xF0, read_reg(0xF0) | 0x100);

        let svr = read_reg(APIC_SVR_OFFSET);
        write_reg(APIC_SVR_OFFSET, svr | 0x100 | (PIC_1_OFFSET as u32));

        write_reg(APIC_TIMER_DIV_OFFSET, 0x0011);
        write_reg(
            APIC_LVT_TIMER_OFFSET,
            APIC_TIMER_PERIODIC | (PIC_1_OFFSET as u32),
        );
        write_reg(APIC_TIMER_INITCNT_OFFSET, 1_000_000_000);
    } else {
        *USING_APIC.lock() = false;
        info!("(PICS) Using PICS!");
    }
}

unsafe fn init_pit(frequency_hz: u32) {
    info!("(PIT) Initializing PIT with frequency: {} Hz", frequency_hz);
    let divisor = (1193182 / frequency_hz) as u16;
    use x86_64::instructions::port::Port;

    let mut command = Port::new(0x43);
    let mut channel0 = Port::new(0x40);

    command.write(0x36u8);

    channel0.write((divisor & 0xFF) as u8);
    channel0.write((divisor >> 8) as u8);
}

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    info!("Breakpoint\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    error!(
        "\nUh-oh! The Lemoncake kernel page-faulted.\nHere's what happened:\nAccessed Address: {:?}\nError Code: {:?}\nStack Frame:\n{:#?}",
        Cr2::read(),
        error_code,
        stack_frame
    );
    
    sad!();

    hlt_loop();
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    error!(
        "\nUh-oh! The Lemoncake kernel double-faulted.\nHere's the stack frame:\n{:#?}\nError Code: {}",
        stack_frame, error_code
    );

    sad!();

    loop {}
}

#[allow(unused)]
extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    serial_print!(".");
    unsafe {
        if *USING_APIC.lock() { apic_eoi(); }
        else { PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8()); }
    }
}

#[allow(unused)]
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    crate::keyboard::add_scancode(scancode);

    unsafe {
        apic_eoi();
    }
}
