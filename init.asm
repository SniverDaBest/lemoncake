section .data
msg db "this is what the can of dr. pepper that rolled down my keyboard wrote:", 0xA, "078956ouiytrlkjh,gfmnb vc", 0xA, 0xA, "also hi from usermode i guess"
len equ $ - msg

section .text
global _start
_start:
    mov rax, 1
    mov rdi, 1
    lea rsi, [rel msg]
    mov rdx, len
    int 0x80

    jmp $
