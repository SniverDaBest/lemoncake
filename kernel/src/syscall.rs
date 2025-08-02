use crate::{error, gdt::GDT, info, nftodo, println, sleep::Sleep, warning};
use core::{arch::asm, str};
use x86_64::{
    registers::{
        control::{Efer, EferFlags},
        model_specific::{LStar, Msr, SFMask, Star},
        rflags::RFlags,
    }, VirtAddr
};

#[unsafe(no_mangle)]
unsafe extern "C" fn syscall_entry() {
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

pub unsafe fn jump_to_usermode(entry: u64, user_stack_top: u64) -> ! {
    asm!(
        "cli",                       // Disable interrupts during transition
        "mov ax, 0x23",       // Load user data selector
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",

        // Push SS, RSP, RFLAGS, CS, RIP (in reverse order for iretq)
        "push 0x23",         // SS
        "push {}",             // RSP
        "pushfq",                    // RFLAGS
        "pop rax",                   // Modify RFLAGS manually if needed
        "or rax, 0x200",             // Ensure IF=1 (interrupts enabled)
        "push rax",
        "push 0x1b",         // CS
        "push {}",             // RIP

        "iretq",
        in(reg) user_stack_top,
        in(reg) entry,
        options(noreturn)
    );
}

pub unsafe fn init_syscalls() {
    Efer::update(|e| *e |= EferFlags::SYSTEM_CALL_EXTENSIONS);
    Msr::new(0xC000_0081).write(((0x1Bu64) << 48) | ((0x08u64) << 32));
    Msr::new(0xC000_0082).write(syscall_entry as *const u64 as u64);
    Msr::new(0xC000_0084).write(RFlags::INTERRUPT_FLAG.bits());
}

pub unsafe fn switch_to_user(entry: u64, user_stack_top: u64) {    
    jump_to_usermode(entry, user_stack_top);
}

pub unsafe fn verify_syscall_msr() {
    let star = Msr::new(0xC000_0081).read();
    let lstar = LStar::read();
    let sfmask = SFMask::read();

    info!("STAR MSR: {:#018x}", star);
    info!("LSTAR MSR: {:#018x}", lstar);
    info!("SFMASK MSR: {:#018x}", sfmask);

    // Decode selectors from STAR:
    let user_cs = (star >> 48) & 0xFFFF;
    let kernel_cs = (star >> 32) & 0xFFFF;

    info!("User CS selector (from STAR): {:#06x}", user_cs);
    info!("Kernel CS selector (from STAR): {:#06x}", kernel_cs);

    // Optionally: read current CS and SS registers (requires inline asm)
    unsafe {
        let cs: u16;
        asm!("mov {0:x}, cs", out(reg) cs);
        info!("Current CS register: {:#06x}", cs);
    }
}