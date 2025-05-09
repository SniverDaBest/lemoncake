use core::arch::asm;

use crate::{error, gdt, hlt_loop, info, serial_print};
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use x86_64::PhysAddr;
use x86_64::VirtAddr;
use x86_64::registers::model_specific::Msr;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use x86_64::structures::paging::PageTableFlags;
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PhysFrame, Size4KiB};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
const IA32_APIC_BASE_MSR: u32 = 0x1B;
const IA32_APIC_BASE_MSR_ENABLE: u32 = 0x800;
const APIC_EOI_OFFSET: usize = 0xB0;

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
        }
        idt[0x20].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);
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
    let msr = Msr::new(IA32_APIC_BASE_MSR);
    let value = msr.read();
    let eax: u32 = value as u32;
    let edx: u32 = (value >> 32) as u32;

    return (eax as usize & 0xfffff000) | ((edx as usize & 0x0f) << 32);
}

unsafe fn read_reg(offset: usize) -> u32 {
    let apic_base = get_apic_base();
    let reg_ptr = (apic_base + offset) as *const u32;
    core::ptr::read_volatile(reg_ptr)
}

unsafe fn write_reg(offset: usize, value: u32) {
    let apic_base = get_apic_base();
    let reg_ptr = (apic_base + offset) as *mut u32;
    core::ptr::write_volatile(reg_ptr, value);
}

fn apic_eoi() {
    unsafe {
        write_reg(APIC_EOI_OFFSET, 0);
    }
}

pub unsafe fn setup_pics(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    PICS.lock().initialize();

    if check_apic() {
        info!("Using the APIC!");
        disable_pics();

        let apic_base = get_apic_base();
        let apic_frame = PhysFrame::containing_address(PhysAddr::new(apic_base as u64));
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        let page = Page::containing_address(VirtAddr::new(apic_base as u64));
        unsafe {
            mapper
                .map_to(page, apic_frame, flags, frame_allocator)
                .expect("Unable to map the APIC memory region!")
                .flush();
        }

        set_apic_base(apic_base);
        write_reg(0xF0, read_reg(0xF0) | 0x100);
    } else {
        panic!("Please upgrade to a VM/Computer with APIC support!");
    }
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

    hlt_loop();
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    error!(
        "\nUh-oh! The Lemoncake kernel double-faulted.\nHere's the stack frame:\n{:#?}",
        stack_frame
    );

    loop {}
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    serial_print!(".");
    if unsafe { check_apic() } {
        apic_eoi();
    } else {
        unsafe {
            PICS.lock()
                .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
        }
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    crate::keyboard::add_scancode(scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
