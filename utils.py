#!/usr/bin/python3
# TODO: Make FreeBSD stuff

import platform
import os
import sys
from pathlib import Path

p = platform.system()

LINUX = "Linux"
WINDOWS = "Windows"
MAC = "Darwin"

print(f"\x1b[34m(o_o) [INFO ]:\x1b[0m Running on {p if p != MAC else 'MacOS'}")

if p != LINUX and p != MAC:
    print(
        "\x1b[33m(0_0) [WARN ]:\x1b[0m The only OS this supports is Linux and (kinda) MacOS.. Windows users can use WSL2, or use a better OS (linux)",
        file=sys.stderr,
    )

os.system(
    "rustup target add x86_64-unknown-none && rustup component add llvm-tools rust-src"
)

print("\x1b[34m(o_o) [INFO ]:\x1b[0m Compiling init program...")
ret = os.system("nasm -felf64 init.asm")
if ret == 0:
    print("\x1b[34m(o_o) [INFO ]:\x1b[0m Successfully compiled the init program!")
    if p != WINDOWS:
        ret = os.system("ld -o init init.o")
        if ret == 0:
            print("\x1b[34m(o_o) [INFO ]:\x1b[0m Successfully linked the init program!")
        elif ret == 127:
            print(
                "\x1b[31m(X_X) [ERROR]:\x1b[0m Please install ld! It's part of binutils.",
                file=sys.stderr,
            )
        else:
            print(
                "\x1b[31m(X_X) [ERROR]:\x1b[0m Failed to link the init prorgam!",
                file=sys.stderr,
            )
    else:
        print(
            "\x1b[31m(X_X) [ERROR]:\x1b[0m Please run this install script under Linux. Windows doesn't have binutils.",
            file=sys.stderr,
        )
elif ret == 127:
    print(
        "\x1b[31m(X_X) [ERROR]:\x1b[0m Please install nasm to compile the init program!",
        file=sys.stderr,
    )
else:
    print(
        "\x1b[31m(X_X) [ERROR]:\x1b[0m Unable to compile the init program!",
        file=sys.stderr,
    )

print("\x1b[34m(o_o) [INFO ]:\x1b[0m Creating new ramdisk...\x1b[0m")
os.system("mkdir -p target/ramdisk")
if not os.path.exists("target/ramdisk/lorem.txt"):
    with open("target/ramdisk/lorem.txt", "w") as f:
        f.write("Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed non.")

if not os.path.exists("target/ramdisk/init"):
    os.system("cp init target/ramdisk/")

output_path = Path("target/ramdisk/lcsrc.txt")
output_path.parent.mkdir(parents=True, exist_ok=True)

src_path = Path("kernel/src")

if not src_path.exists():
    print(
        "\x1b[31m(X_X) [ERROR]:\x1b[0m Couldn't read kernel source code!",
        file=sys.stderr,
    )
    sys.exit(1)

files = sorted([f for f in src_path.rglob('*') if f.is_file()])

if not files:
    print(f"No files found in {"kernel/src"}")
    sys.exit(1)

with open("target/ramdisk/lcsrc.txt", 'w', encoding='utf-8') as out:
    for file_path in files:
        rel_path = file_path.relative_to(src_path)
        out.write(f"// {rel_path}\n")
        
        try:
            with open(file_path, 'r', encoding='utf-8') as f:
                content = f.read()
                out.write(content)
                
                if content and not content.endswith('\n'):
                    out.write('\n')
                out.write('\n')
        except Exception as e:
            out.write(f"// Error reading file: {e}\n\n")
            print(f"Warning: Could not read {file_path}: {e}")            

os.system("cd target/ramdisk/ && tar cvf ../../hd.tar *")