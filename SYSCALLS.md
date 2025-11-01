>[!CRITICAL]
> Syscall 3 (wait) is broken!\
> Syscall 7 & 8 are just stubs!

>[!IMPORTANT]
> All of these are bound to change! Maybe one day I'll make syscall 2 `inttostr` and make syscall 5 `panic`. Who knows, though?

| Name | Description | ID (rax) | rdi | rsi | rdx | r10 | r8 | r9 |
| :---- | :---- | :---- | :---- | :---- | :---- | :---- | :---- | :---- |
| read | Reads from a file. | 0 | Pointer to filename | Length of filename | Pointer to buffer | Offset | Amount to read (u64::MAX is entire file) | N/A |
| write | Writes to stdout. | 1 | Formatting style. 1 is Normal, 2 is Info, 3 is Warning, 4 is Error, and 5 is TODO | Pointer to text | Length of text | N/A | N/A | N/A |
| panic | Makes the kernel panic with a message. | 2 | Pointer to text | Length of text | N/A | N/A | N/A | N/A
| wait | Makes the kernel wait for X amount of milliseconds. | 3 | Milliseconds | N/A | N/A | N/A | N/A | N/A
| randu64 | Generates a random value. | 4 | N/A | N/A | N/A | N/A | N/A | N/A
| inttostr | Generates a random value. | 5 | Buffer | Buffer Size | Number | N/A | N/A | N/A
| emoticon | Displays a sad or happy face. | 6 | Sad/Happy | N/A | N/A | N/A | N/A | N/A
| listdir | Lists all files in a directory. | 7 | Pointer to buffer | N/A | N/A | N/A | N/A | N/A
| filesize | Gets a file's size | 8 | Pointer to filename | Length of filename | Pointer to size | N/A | N/A | N/A |

# Examples
>[!IMPORTANT]
> This is NASM assembly. I will not write FASM, GAS, or any other sort of assembly anywhere in this project.
To compile any of these examples, you will need to be on some sort of *nix (Yes, that includes MacOS and BSD).\
If you need to compile them, you'll need these things:
- nasm
- ld (binutils)
You can run the following commands to build one:
```bash
nasm some_example.asm -felf64
ld some_example.o -o some_example
```
From there, you can change `kernel/src/main.rs` to load that example instead of the init program.

## `write` syscall
```asm
; write syscall

section .data ; .data stores all of the variables and other data.
msg db "Hello, World!", 0xA ; you can put any text here. 0xA is a newline.
len equ $ - msg ; gets the length of the message

section .text ; .text is the code section
global _start ; make _start (the entrypoint) global
_start:
    mov rax, 1 ; #1 is the ID for the write syscall
    mov rdi, 1 ; 1 is the identifier for standard printing.
    lea rsi, [rel msg] ; put the message in rsi
    mov rdx, len ; put the length into rdx
    int 0x80 ; call the syscall

    jmp $ ; loop forever. nothing to do.
```

## `emoticon` syscall
```asm
; emoticon syscall

section .text ; .text is the code section
global _start ; make _start (the entrypoint) global
_start:
    mov rax, 6 ; #6 is the ID for the emoticon syscall
    mov rdi, 1 ; smiley
    int 0x80 ; call the syscall
    
    mov rax, 6 ; #6 is the ID for the emoticon syscall
    mov rdi, 2 ; sad
    int 0x80 ; call the syscall

    jmp $ ; loop forever. nothing to do.
```