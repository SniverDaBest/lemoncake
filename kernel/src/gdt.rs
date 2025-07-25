use lazy_static::lazy_static;
use x86_64::VirtAddr;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

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
            stack_end
        };

        tss.privilege_stack_table[0] = {
            const KSTACK_SIZE: usize = 4096 * 5;
            #[repr(align(16))]
            struct KStack([u8; KSTACK_SIZE]);
            static mut KSTACK: KStack = KStack([0; KSTACK_SIZE]);
            #[allow(static_mut_refs)]
            let stack_start = VirtAddr::from_ptr(unsafe { &KSTACK.0 as *const _ });
            let stack_end = stack_start + KSTACK_SIZE as u64;
            stack_end
        };

        tss
    };
}

lazy_static! {
    pub static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
        let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());
        let user_code_selector = gdt.append(Descriptor::user_code_segment());
        let user_data_selector = gdt.append(Descriptor::user_data_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));

        (
            gdt,
            Selectors {
                kernel_code_selector,
                user_code_selector,
                kernel_data_selector,
                user_data_selector,
                tss_selector,
            },
        )
    };
}

pub struct Selectors {
    pub kernel_code_selector: SegmentSelector,
    pub user_code_selector: SegmentSelector,
    pub kernel_data_selector: SegmentSelector,
    pub user_data_selector: SegmentSelector,
    pub tss_selector: SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::segmentation::{CS, DS, ES, SS, Segment};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.kernel_code_selector);
        DS::set_reg(GDT.1.kernel_data_selector);
        ES::set_reg(GDT.1.kernel_data_selector);
        SS::set_reg(GDT.1.kernel_data_selector);
        load_tss(GDT.1.tss_selector);
    }
}
