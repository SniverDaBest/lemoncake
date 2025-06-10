use core::sync::atomic::{AtomicU64, Ordering};
use lazy_static::lazy_static;
use x86_64::VirtAddr;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

use crate::info;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
pub const TIMER_IST_INDEX: u16 = 1;

static DF_STACK_START: AtomicU64 = AtomicU64::new(0);
static DF_STACK_END: AtomicU64 = AtomicU64::new(0);
static TIMER_STACK_START: AtomicU64 = AtomicU64::new(0);
static TIMER_STACK_END: AtomicU64 = AtomicU64::new(0);

pub fn double_fault_stack_bounds() -> (VirtAddr, VirtAddr) {
    (
        VirtAddr::new(DF_STACK_START.load(Ordering::Relaxed)),
        VirtAddr::new(DF_STACK_END.load(Ordering::Relaxed)),
    )
}

pub fn timer_stack_bounds() -> (VirtAddr, VirtAddr) {
    (
        VirtAddr::new(TIMER_STACK_START.load(Ordering::Relaxed)),
        VirtAddr::new(TIMER_STACK_END.load(Ordering::Relaxed)),
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
            #[allow(static_mut_refs)]
            let stack_start = VirtAddr::from_ptr(unsafe { &STACK.0 as *const _ });
            let stack_end = stack_start + STACK_SIZE as u64;
            DF_STACK_START.store(stack_start.as_u64(), Ordering::Relaxed);
            DF_STACK_END.store(stack_end.as_u64(), Ordering::Relaxed);
            stack_end
        };
        tss.interrupt_stack_table[TIMER_IST_INDEX as usize] = {
            pub const STACK_SIZE: usize = 4096 * 5;
            #[repr(align(16))]
            pub struct Stack([u8; STACK_SIZE]);
            pub static mut STACK: Stack = Stack([0; STACK_SIZE]);
            #[allow(static_mut_refs)]
            let stack_start = VirtAddr::from_ptr(unsafe { &STACK.0 as *const _ });
            let stack_end = stack_start + STACK_SIZE as u64;
            TIMER_STACK_START.store(stack_start.as_u64(), Ordering::Relaxed);
            TIMER_STACK_END.store(stack_end.as_u64(), Ordering::Relaxed);
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

        let (df_start, df_end) = double_fault_stack_bounds();
        let (timer_start, timer_end) = timer_stack_bounds();
        info!(
            "(GDT) Double Fault IST Stack Range: {:#x} - {:#x}",
            df_start.as_u64(),
            df_end.as_u64()
        );
        info!(
            "(GDT) Timer IST Stack Range: {:#x} - {:#x}",
            timer_start.as_u64(),
            timer_end.as_u64()
        );
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
    use x86_64::instructions::segmentation::{SS, CS, Segment};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);
    }

    info!("(GDT) CS: {:?}", CS::get_reg());
    info!("(GDT) SS: {:?}", SS::get_reg());
}
