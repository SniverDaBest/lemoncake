extern kernel_main
global start

section .boot
bits 32
start:
    mov [.mb2_magic], eax
    mov [.mb2_info], ebx

    call check_mb2

    ; Point the first entry of the level 4 page table to the first entry in the
    ; p3 table
    mov eax, p3_table
    or eax, 0b11 ; 
    mov dword [p4_table + 0], eax

    ; Point the first entry of the level 3 page table to the first entry in the
    ; p2 table
    mov eax, p2_table
    or eax, 0b11
    mov dword [p3_table + 0], eax

    ; point each page table level two entry to a page
    mov ecx, 0         ; counter variable
.map_p2_table:
    mov eax, 0x200000  ; 2MiB
    mul ecx
    or eax, 0b10000011
    mov [p2_table + ecx * 8], eax

    inc ecx
    cmp ecx, 512
    jne .map_p2_table

    ; Move Page Table Addr to CR3
    mov eax, p4_table
    mov cr3, eax

    ; enable PAE
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    ; Set Long Mode Bit
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    ; Enable Paging
    mov eax, cr0
    or eax, (1 << 31 | 1 << 16)
    mov cr0, eax

    lgdt [gdt64.pointer]

    mov ax, gdt64.data
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov gs, ax
    mov fs, ax

    jmp gdt64.code:startos

    .mb2_magic: 
        dd 0
    .mb2_info:
        dq 0

    ; shouldn't ever happen
    hlt

check_mb2:
    mov eax, [start.mb2_magic]
    cmp eax, 0x36D76289
    jne .fail
    ret
    .fail:
        hlt

bits 64
startos:
    mov rsi, [start.mb2_magic]
    mov rdi, [start.mb2_info]
    call kernel_main

section .bss
align 4096
p4_table:
    resb 4096
p3_table:
    resb 4096
p2_table:
    resb 4096

section .rodata
gdt64:
    dq 0 ; zero entry
.code: equ $ - gdt64
    dq (1<<44) | (1<<47) | (1<<41) | (1<<43) | (1<<53) ; code segment
.data: equ $ - gdt64
    dq (1<<44) | (1<<47) | (1<<41) ; data segment
.pointer:
    dw $ - gdt64 - 1
    dq gdt64