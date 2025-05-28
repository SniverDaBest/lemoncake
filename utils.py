import platform
import os
import sys

p = platform.system()

LINUX = "Linux"
WINDOWS = "Windows"
MAC = "Darwin"

print(f"(o_o) INFO: Running on {p if p != MAC else "MacOS"}")

if p != LINUX: print(f"(0_o) WARNING: The only OS this supports is Linux. Windows users can use WSL2, and MacOS users can... figure that out themselves.", file=sys.stderr)

os.system("rustup target add x86_64-unknown-none && rustup component add llvm-tools rust-src")

if not os.path.exists("hd.img"):
    print("Creating new HD image...")
    if p == WINDOWS: os.system("qemu-img create hd.img 512M")
    elif p == LINUX or p == MAC: os.system("dd if=/dev/zero of=hd.img bs=1M count=512")

    if p == LINUX: os.system("mkfs.fat -F 32 hd.img") # idfk if MacOS has this, so it's going to be linux exclusive. (i don't have a mac; i'm broke)
    else: print("Unsupported platform. Can't format HD image!", file=sys.stderr)
else: print("HD image already exists.")