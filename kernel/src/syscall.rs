use crate::{error, info, nftodo, print, rdrand, sad, sleep::Sleep, warning, yay};
use alloc::format;
use core::{
    arch::{asm, naked_asm},
    str,
};
use lazy_static::lazy_static;
use x86_64::{VirtAddr, structures::idt::InterruptStackFrame};

const MAX_PRINT: usize = 4096;

lazy_static! {
    static ref SYSCALL_STACK: VirtAddr = {
        const KSTACK_SIZE: usize = 4096 * 5;
        #[repr(align(16))]
        struct KStack([u8; KSTACK_SIZE]);
        static mut KSTACK: KStack = KStack([0; KSTACK_SIZE]);
        #[allow(static_mut_refs)]
        let stack_start = VirtAddr::from_ptr(unsafe { &KSTACK.0 as *const _ });
        stack_start + KSTACK_SIZE as u64
    };
}

#[unsafe(no_mangle)]
#[allow(unused)]
pub unsafe fn syscall_handler(
    rax: usize,
    rdi: usize,
    rsi: usize,
    rdx: usize,
    r10: usize,
    r8: usize,
    r9: usize,
) -> usize {
    match rax {
        1 => {
            if rdx == 0 || rdx > MAX_PRINT || rsi == 0 {
                error!("(SYSCALL) Bad print args! (Length {} at {:#x})", rdx, rsi);
                return usize::MAX;
            }

            let bytes = core::slice::from_raw_parts(rsi as *const u8, rdx);
            if let Ok(s) = core::str::from_utf8(bytes) {
                match rdi {
                    1 => print!("{}", s),
                    2 => info!("{}", s),
                    3 => warning!("{}", s),
                    4 => error!("{}", s),
                    5 => nftodo!("{}", s),
                    _ => return usize::MAX,
                }
            } else {
                warning!("Seemingly garbage data at {:#x}. Size: {} bytes", rsi, rdx);
            }
            return rdx;
        }
        2 => {
            panic!("{}", str::from_raw_parts(rdi as *const u8, rsi));
        }
        3 => {
            Sleep::ms(rdi as u64);
            return 0;
        }
        4 => {
            for i in 0..25 {
                let r = rdrand();
                if r.is_none() {
                    continue;
                }

                return r.unwrap() as usize;
            }
            return usize::MAX;
        }
        5 => {
            let istr_fmt = format!("{}", rdx);
            let istr = istr_fmt.as_bytes();

            let to_write = core::cmp::min(istr.len(), rsi);
            if to_write == 0 {
                return usize::MAX;
            }

            let dst = core::slice::from_raw_parts_mut(rdi as *mut u8, to_write);
            dst.copy_from_slice(&istr[..to_write]);

            return to_write;
        }
        6 => match rdi {
            1 => {
                yay!();
                return 0;
            }
            2 => {
                sad!();
                return 0;
            }
            _ => return usize::MAX,
        },
        i => {
            error!("(SYSCALL) Invalid syscall number {}!", i);
            return usize::MAX;
        }
    }
}

pub unsafe fn jump_to_usermode(entry: u64, user_stack_top: u64) -> ! {
    asm!(
        "mov ax, 0x23",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",

        "push 0x23",
        "push {}",
        "pushfq",
        "pop rax",
        "or rax, 0x200",
        "push rax",
        "push 0x1b",
        "push {}",

        "iretq",
        in(reg) user_stack_top,
        in(reg) entry,
        options(noreturn)
    );
}

#[repr(C)]
pub struct Regs {
    pub rax: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub r10: u64,
    pub r8: u64,
    pub r9: u64,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall_from_stub(
    regs_ptr: *mut Regs,
    _frame: *mut InterruptStackFrame,
) -> u64 {
    let regs = unsafe { &mut *regs_ptr };

    let ret = syscall_handler(
        regs.rax as usize,
        regs.rdi as usize,
        regs.rsi as usize,
        regs.rdx as usize,
        regs.r10 as usize,
        regs.r8 as usize,
        regs.r9 as usize,
    );

    ret as u64
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn int80_entry() -> ! {
    naked_asm!(
        "push r9",
        "push r8",
        "push r10",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rax",
        "mov rdi, rsp",
        "lea rsi, [rsp + 56]",
        "call {}",
        "add rsp, 56",
        "iretq",

        sym syscall_from_stub,
    )
}
