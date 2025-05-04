import platform
import os
import sys

p = platform.system()

LINUX = "Linux"
WINDOWS = "Windows"
MAC = "Darwin"

print(f"(o_o) INFO: Running on {p if p != "Darwin" else "MacOS"}")

if p != LINUX: print(f"(0_o) WARNING: The only OS this supports is Linux. Windows users can use WSL2, and MacOS users can... figure that out themselves.", file=sys.stderr)

os.system("rustup target add x86_64-unknown-none")
os.system("rustup component add llvm-tools rust-src")

if not os.path.exists("hd.img"):
    print("Creating new HD image...")
    os.system("qemu-img create hd.img 512M")
    if p == LINUX: os.system("mkfs.fat -F 32 hd.img")
    else: print("Unsupported platform. Can't format HD image!")
else: print("HD image already exists.")