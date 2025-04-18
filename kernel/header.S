.section .kboot_hdr, "a", @progbits
    .ascii  "kb00t!"       # 6 bytes
    .short  1        # 2 bytes
    .quad   kmain  # 8 bytes
