.extern kernel_main
.global start

.section .boot
.code32
start:
    movl %eax, mb2_magic
    movl %ebx, mb2_info

    call check_mb2

    # Point the first entry of the level 4 page table to the first entry in the p3 table
    movl $p3_table, %eax
    orl $0b11, %eax
    movl %eax, p4_table

    # Point the first entry of the level 3 page table to the first entry in the p2 table
    movl $p2_table, %eax
    orl $0b11, %eax
    movl %eax, p3_table

    # Point each page table level two entry to a page
    xorl %ecx, %ecx         # counter variable
map_p2_table:
    movl $0x200000, %eax  # 2MiB
    mull %ecx
    orl $0b10000011, %eax
    movl %eax, p2_table(,%ecx,8)

    incl %ecx
    cmpl $512, %ecx
    jne map_p2_table

    # Move Page Table Addr to CR3
    movl $p4_table, %eax
    movl %eax, %cr3

    # Enable PAE
    movl %cr4, %eax
    orl $(1 << 5), %eax
    movl %eax, %cr4

    # Set Long Mode Bit
    movl $0xC0000080, %ecx
    rdmsr
    orl $(1 << 8), %eax
    wrmsr

    # Enable Paging
    movl %cr0, %eax
    orl $((1 << 31) | (1 << 16)), %eax
    movl %eax, %cr0

    lgdt gdt64_pointer

    movw $gdt64_data, %ax
    movw %ax, %ss
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %gs
    movw %ax, %fs

    ljmp $gdt64_code, $startos

    .balign 4
mb2_magic: 
    .long 0
mb2_info:
    .quad 0

    # shouldn't ever happen
    hlt

check_mb2:
    movl mb2_magic, %eax
    cmpl $0x36D76289, %eax
    jne fail
    ret
fail:
    hlt

.code64
startos:
    movq mb2_magic(%rip), %rsi
    movq mb2_info(%rip), %rdi
    call kernel_main

.section .bss
.balign 4096
p4_table:
    .skip 4096
p3_table:
    .skip 4096
p2_table:
    .skip 4096

.section .rodata
gdt64:
    .quad 0 # zero entry
gdt64_code = . - gdt64
    .quad (1<<44) | (1<<47) | (1<<41) | (1<<43) | (1<<53) # code segment
gdt64_data = . - gdt64
    .quad (1<<44) | (1<<47) | (1<<41) # data segment
gdt64_pointer:
    .word . - gdt64 - 1
    .quad gdt64
