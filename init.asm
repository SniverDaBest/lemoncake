section .data
msg db "Orange moss potato. Key pasta carrot?", 0xA ; what the hell was i thinking when i wrote this?
                                                    ; i mean, it's funny, so i'm 100% keeping this for a while
                                                    ; but i also kinda wanna know what was going through my head when i wrote this...
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
