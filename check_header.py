import sys
path = sys.argv[1]

with open(path, 'rb') as f:
    data = f.read()
    f.close()

correct = True

if len(data) < 13:
    print("[✘] File too small")
    correct = False
else:
    print("[✔] Header size is valid")

if data.startswith(b"kb00t!"):
    print("[✔] Found magic")
else:
    print("[✘] Magic not found")
    correct = False

if data[6:7] == b"\x01":
    print("[✔] Found version: 01")
elif data[6:7] == 0x0000:
    print("[✘] Version not found")
    correct = False
else:
    print(f"[✘] Found incorrect version: " + str(data[6:7]).lstrip("b'\\x").rstrip("'"))
    correct = False

if data[8:12] != b"\x00\x00\x00\x00":
    entry_point = int.from_bytes(data[8:12], byteorder='big')
    print(f"[✔] Found kernel entry point address: {entry_point:#010x}")
else:
    print("[✘] Entry point address not found")
    correct = False

if not correct: sys.exit(1)
