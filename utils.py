import platform
import os
import sys

p = platform.system()

LINUX = "Linux"
WINDOWS = "Windows"
MAC = "Darwin"

print(f"(o_o) INFO: Running on {p if p != "Darwin" else "MacOS"}")

if p == MAC: print(f"(0_o) WARNING: Although it probably will still work, MacOS is NOT supported!", file=sys.stderr)

os.system("rustup target add x86_64-unknown-none")
os.system("rustup component add llvm-tools rust-src")

if not os.path.exists("hd.img"):
    print("Creating new HD image...")
    os.system("qemu-img create hd.img 512M")

    with open("hd.img", "wb") as f:
        f.write(b"SHFS!I love kittens!!\x01\x00\x20\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00") # sample data. won't actually work, but it has the fields.
        f.close()
else: print("HD image already exists.")