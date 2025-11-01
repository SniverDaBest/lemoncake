section .data
msg db 0xA, "Successfully entered init program!", 0xA
len equ $ - msg
space db ' '

section .text
global _start
_start:
    mov rax, 6
    mov rdi, 1
    int 0x80
    
    mov rax, 1
    mov rdi, 1
    lea rsi, [rel space]
    mov rdx, 1
    int 0x80
    
    mov rax, 6
    mov rdi, 2
    int 0x80

    mov rax, 1
    mov rdi, 1
    lea rsi, [rel msg]
    mov rdx, len
    int 0x80

    jmp $
