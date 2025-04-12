ENTRY(kmain)

SECTIONS {
    . = 0x0;

    .kboot_hdr : {
        KEEP(*(.kboot_hdr))
    }

    _kmain_addr = kmain;

    . = 0x1000;
    .text : {
        *(.text .text.*)
    }
    .rodata : {
        *(.rodata .rodata.*)
    }
    .data : {
        *(.data .data.*)
    }
    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    }
}
