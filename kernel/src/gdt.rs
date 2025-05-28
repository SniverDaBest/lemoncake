use core::ptr::addr_of;
use core::sync::atomic::{AtomicU64, Ordering};
use lazy_static::lazy_static;
use x86_64::VirtAddr;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
pub const TIMER_IST_INDEX: u16 = 1;

static STACK_START: AtomicU64 = AtomicU64::new(0);
static STACK_END: AtomicU64 = AtomicU64::new(0);

pub fn tss_stack_bounds() -> (VirtAddr, VirtAddr) {
    (
        VirtAddr::new(STACK_START.load(Ordering::Relaxed)),
        VirtAddr::new(STACK_END.load(Ordering::Relaxed)),
    )
}

lazy_static! {
    pub static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            pub const STACK_SIZE: usize = 4096 * 5;

            #[repr(align(16))]
            pub struct Stack([u8; STACK_SIZE]);

            pub static mut STACK: Stack = Stack([0; STACK_SIZE]);

            let x = unsafe { STACK.0 };

            let stack_start = VirtAddr::from_ptr(addr_of!(x));
            let stack_end = stack_start + STACK_SIZE as u64;

            STACK_START.store(stack_start.as_u64(), Ordering::Relaxed);
            STACK_END.store(stack_end.as_u64(), Ordering::Relaxed);

            stack_end
        };
        tss.interrupt_stack_table[TIMER_IST_INDEX as usize] = {
            pub const STACK_SIZE: usize = 4096 * 5;
            #[repr(align(16))]
            pub struct Stack([u8; STACK_SIZE]);
            pub static mut STACK: Stack = Stack([0; STACK_SIZE]);
            let x = unsafe { STACK.0 };
            let stack_start = VirtAddr::from_ptr(addr_of!(x));
            let stack_end = stack_start + STACK_SIZE as u64;
            stack_end
        };
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.append(Descriptor::kernel_code_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
        (
            gdt,
            Selectors {
                code_selector,
                tss_selector,
            },
        )
    };
}

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::segmentation::{CS, Segment};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);
    }
}
