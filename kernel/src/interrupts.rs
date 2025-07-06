use crate::{error, gdt, hlt_loop, info, serial_print};
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spinning_top::Spinlock;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
pub const USING_APIC: Spinlock<bool> = Spinlock::new(true);

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

        idt[50 + 0].set_handler_fn(ioapic_handler_0);
        idt[50 + 1].set_handler_fn(ioapic_handler_1);
        idt[50 + 2].set_handler_fn(ioapic_handler_2);
        idt[50 + 3].set_handler_fn(ioapic_handler_3);
        idt[50 + 4].set_handler_fn(ioapic_handler_4);
        idt[50 + 5].set_handler_fn(ioapic_handler_5);
        idt[50 + 6].set_handler_fn(ioapic_handler_6);
        idt[50 + 7].set_handler_fn(ioapic_handler_7);
        idt[50 + 8].set_handler_fn(ioapic_handler_8);
        idt[50 + 9].set_handler_fn(ioapic_handler_9);
        idt[50 + 10].set_handler_fn(ioapic_handler_10);
        idt[50 + 11].set_handler_fn(ioapic_handler_11);
        idt[50 + 12].set_handler_fn(ioapic_handler_12);
        idt[50 + 13].set_handler_fn(ioapic_handler_13);
        idt[50 + 14].set_handler_fn(ioapic_handler_14);
        idt[50 + 15].set_handler_fn(ioapic_handler_15);
        idt[50 + 16].set_handler_fn(ioapic_handler_16);
        idt[50 + 17].set_handler_fn(ioapic_handler_17);
        idt[50 + 18].set_handler_fn(ioapic_handler_18);
        idt[50 + 19].set_handler_fn(ioapic_handler_19);
        idt[50 + 20].set_handler_fn(ioapic_handler_20);
        idt[50 + 21].set_handler_fn(ioapic_handler_21);
        idt[50 + 22].set_handler_fn(ioapic_handler_22);
        idt[50 + 23].set_handler_fn(ioapic_handler_23);

        idt
    };
}

pub unsafe fn disable_pics() {
    PICS.lock().initialize();
    PICS.lock().write_masks(0xFF, 0xFF);
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
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

extern "x86-interrupt" fn seg_fault_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    error!(
        "\nUh-oh! The Lemoncake kernel segfaulted.\nHere's the stack frame:\n{:#?}\nError Code: {}",
        stack_frame, error_code
    );

    if let Some(tty) = crate::TTY.lock().as_mut() {
        tty.sad(Some((243, 139, 168, 255)));
    }
    #[cfg(feature = "serial-faces")]
    serial_print!("☹"); // this may not render in all terminals! disable the `serial-faces` feature to get rid of it.

    loop {}
}
