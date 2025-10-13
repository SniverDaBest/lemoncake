use crate::{error, gdt, hlt_loop, info, serial_print};
use core::sync::atomic::{AtomicU64, Ordering};
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spinning_top::Spinlock;
use x86_64::{
    VirtAddr,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
pub static USING_APIC: Spinlock<bool> = Spinlock::new(true);
pub static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.general_protection_fault
            .set_handler_fn(gp_fault_handler);

        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }

        for i in 32..=255 {
            idt[i].set_handler_fn(generic_interrupt_handler);
        }

        idt[48].set_handler_fn(timer_interrupt_handler);
        idt[49].set_handler_fn(timer_interrupt_handler2);

        idt[50].set_handler_fn(ioapic_handler_0);
        idt[51].set_handler_fn(ioapic_handler_1);
        idt[52].set_handler_fn(ioapic_handler_2);
        idt[53].set_handler_fn(ioapic_handler_3);
        idt[54].set_handler_fn(ioapic_handler_4);
        idt[55].set_handler_fn(ioapic_handler_5);
        idt[56].set_handler_fn(ioapic_handler_6);
        idt[57].set_handler_fn(ioapic_handler_7);
        idt[58].set_handler_fn(ioapic_handler_8);
        idt[59].set_handler_fn(ioapic_handler_9);
        idt[60].set_handler_fn(ioapic_handler_10);
        idt[61].set_handler_fn(ioapic_handler_11);
        idt[62].set_handler_fn(ioapic_handler_12);
        idt[63].set_handler_fn(ioapic_handler_13);
        idt[64].set_handler_fn(ioapic_handler_14);
        idt[65].set_handler_fn(ioapic_handler_15);
        idt[66].set_handler_fn(ioapic_handler_16);
        idt[67].set_handler_fn(ioapic_handler_17);
        idt[68].set_handler_fn(ioapic_handler_18);
        idt[69].set_handler_fn(ioapic_handler_19);
        idt[70].set_handler_fn(ioapic_handler_20);
        idt[71].set_handler_fn(ioapic_handler_21);
        idt[72].set_handler_fn(ioapic_handler_22);
        idt[73].set_handler_fn(ioapic_handler_23);

        unsafe {
            idt[0x80]
                .set_handler_addr(VirtAddr::new(
                    crate::syscall::int80_entry as *const u64 as u64,
                ))
                .set_privilege_level(x86_64::PrivilegeLevel::Ring3);
        }

        idt
    };
}

pub unsafe fn disable_pics() {
    PICS.lock().initialize();
    PICS.lock().write_masks(0xFF, 0xFF);
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    TICK_COUNT.store(TICK_COUNT.load(Ordering::Relaxed) + 1, Ordering::Relaxed);

    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}

extern "x86-interrupt" fn timer_interrupt_handler2(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}

extern "x86-interrupt" fn generic_interrupt_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}

extern "x86-interrupt" fn ioapic_handler_0(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_1(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let sc: u8 = unsafe { port.read() };

    crate::keyboard::add_scancode(sc);

    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_2(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_3(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_4(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_5(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_6(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_7(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_8(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_9(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_10(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_11(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_12(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_13(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_14(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_15(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_16(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_17(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_18(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_19(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_20(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_21(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_22(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
    }
}
extern "x86-interrupt" fn ioapic_handler_23(_stack_frame: InterruptStackFrame) {
    unsafe {
        crate::apic::LAPIC
            .get()
            .expect("(APIC) Unable to get the LAPIC!")
            .eoi();
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

    if let Some(tty) = crate::TTY.lock().as_mut() {
        tty.sad(Some((243, 139, 168, 255)));
    }
    #[cfg(feature = "serial-faces")]
    serial_print!("☹"); // this may not render in all terminals! disable the `serial-faces` feature to get rid of it.

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

    if let Some(tty) = crate::TTY.lock().as_mut() {
        tty.sad(Some((243, 139, 168, 255)));
    }
    #[cfg(feature = "serial-faces")]
    serial_print!("☹"); // this may not render in all terminals! disable the `serial-faces` feature to get rid of it.

    loop {}
}

extern "x86-interrupt" fn gp_fault_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    error!(
        "\nUh-oh! The Lemoncake kernel GP-faulted.\nHere's the stack frame:\n{:#?}\nError Code: {}",
        stack_frame, error_code
    );

    if let Some(tty) = crate::TTY.lock().as_mut() {
        tty.sad(Some((243, 139, 168, 255)));
    }
    #[cfg(feature = "serial-faces")]
    serial_print!("☹"); // this may not render in all terminals! disable the `serial-faces` feature to get rid of it.

    loop {}
}
