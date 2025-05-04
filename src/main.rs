fn main() {
    let up = env!("UEFI_PATH");
    let bp = env!("BIOS_PATH");

    println!("UEFI Path: {}\nBIOS Path: {}", up, bp);

    let uefi = true;

    #[cfg(target_os = "linux")]
    let mut copy = std::process::Command::new("cp");

    #[cfg(target_os = "windows")]
    let mut copy = std::process::Command::new("copy");

    if uefi {
        copy.arg(up).arg("./target/uefi.img");
    } else {
        copy.arg(bp).arg("./target/bios.img");
    }

    let mut cpchild = copy.spawn().unwrap();
    cpchild.wait().unwrap();

    let mut cmd = std::process::Command::new("qemu-system-x86_64");

    if uefi {
        cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());
        cmd.arg("-drive").arg(format!("format=raw,file={}", up));
    } else {
        cmd.arg("-drive").arg(format!("format=raw,file={}", bp));
    }

    cmd.arg("-m").arg("1G");
    cmd.arg("-enable-kvm");
    cmd.arg("-serial").arg("stdio");
    //cmd.arg("-vga").arg("cirrus");
    cmd.arg("-drive")
        .arg("id=disk,file=hd.img,if=none,format=raw");
    cmd.arg("-device").arg("ahci,id=ahci");
    cmd.arg("-device").arg("ide-hd,drive=disk,bus=ahci.0");

    let mut child = cmd.spawn().unwrap();
    child.wait().unwrap();
}
