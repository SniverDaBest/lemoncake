# TODO: Make FreeBSD stuff

import platform
import os
import sys

p = platform.system()

LINUX = "Linux"
WINDOWS = "Windows"
MAC = "Darwin"

print(f"\x1b[34m(o_o) [INFO ]: Running on {p if p != MAC else 'MacOS'}")

if p != LINUX and p != MAC: print(f"\x1b[33m(0_0) [WARN ]: The only OS this supports is Linux and (kinda) MacOS.. Windows users can use WSL2, or use a better OS (linux)", file=sys.stderr)

os.system("rustup target add x86_64-unknown-none && rustup component add llvm-tools rust-src")

if not os.path.exists("hd.img"):
    print("\x1b[34m(o_o) [INFO ]: Creating new HD image...\x1b[0m")
    if p == WINDOWS: os.system("qemu-img create hd.img 512M")
    elif p == LINUX or p == MAC: os.system("dd if=/dev/zero of=hd.img bs=1M count=512")

    if p != WINDOWS: os.system("mkfs.fat -F 32 hd.img") # apparently macos does have mkfs.fat :D
    else: print("\x1b[31m(X_X) [ERROR]: Unsupported platform. Can't format HD image!", file=sys.stderr)
else: print("\x1b[34m(o_o) [INFO ]: HD image already exists.")

if not os.path.exists("init"):
    print("\x1b[34m(o_o) [INFO ]: Compiling init program...")
    ret = os.system("nasm -felf64 init.asm")
    if ret == 0:
        print("\x1b[34m(o_o) [INFO ]: Successfully compiled the init program!")
        if p != WINDOWS:
            ret = os.system("ld -o init init.o")
            if ret != 0: print("\x1b[31m(X_X) [ERROR]: Failed to link the init prorgam!", file=sys.stderr) # i'm 99% sure everyone (who isn't on windows) has ld
            else: print("\x1b[34m(o_o) [INFO ]: Successfully linked the init program!")
    elif ret == 127:
        print("\x1b[31m(X_X) [ERROR]: Please install nasm to compile the init program!", file=sys.stderr)
    else:
        print("\x1b[31m(X_X) [ERROR]: Unable to compile the init program!", file=sys.stderr)
else: print("\x1b[34m(o_o) [INFO ]: Init program already compiled.")

