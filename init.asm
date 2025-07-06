section .data
msg db "fish", 0xA
len equ $ - msg

section .text
global _start
_start:
    mov rax, 1
    mov rdi, 1
    mov rsi, msg
    mov rdx, len
    syscall

    mov rax, 1
    mov rdi, msg
    mov rsi, len
    syscall

