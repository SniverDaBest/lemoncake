use crate::{error, gdt::GDT, info, nftodo, println, sleep::Sleep, warning};
use core::{arch::asm, str};
use x86_64::{
    VirtAddr,
    registers::{
        control::{Efer, EferFlags},
        model_specific::{LStar, SFMask, Star},
        rflags::RFlags,
    },
};

#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_entry() {
    info!("Syscall entry called!");
    asm!(
        "swapgs",
        "mov rdi, rax",
        "call syscall_handler",
        "swapgs",
        "sysretq"
    );
}

#[unsafe(no_mangle)]
#[allow(unused)]
unsafe extern "C" fn syscall_handler(
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
            match rdi {
                1 => println!("{}", str::from_raw_parts(rsi as *const u8, rdx)),
                2 => info!("{}", str::from_raw_parts(rsi as *const u8, rdx)),
                3 => warning!("{}", str::from_raw_parts(rsi as *const u8, rdx)),
                4 => error!("{}", str::from_raw_parts(rsi as *const u8, rdx)),
                5 => nftodo!("{}", str::from_raw_parts(rsi as *const u8, rdx)),
                _ => return usize::MAX,
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
        i => {
            error!("(SYSCALL) Invalid syscall number {}!", i);
            return usize::MAX;
        }
    }
}

pub unsafe fn jump_to_usermode(entry: u64, user_stack: u64) {
    asm!(
        "mov rcx, {entry}",
        "mov rsp, {stack}",
        "mov r11, 0x202",
        "sysretq",
        entry = in(reg) entry,
        stack = in(reg) user_stack,
        options(noreturn)
    );
}

pub unsafe fn init_syscalls() {
    Efer::write(Efer::read() | EferFlags::SYSTEM_CALL_EXTENSIONS);
    Star::write_raw(
        GDT.1.user_code_selector.0 - 16,
        GDT.1.kernel_code_selector.0,
    );
    LStar::write(VirtAddr::new(syscall_entry as *const u64 as u64));
    SFMask::write(RFlags::INTERRUPT_FLAG);
}
