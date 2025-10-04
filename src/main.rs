fn main() {
    let up = env!("UEFI_PATH");
    let bp = env!("BIOS_PATH");

    println!("UEFI Path: {}\nBIOS Path: {}", up, bp);

    let uefi = true;

    #[cfg(not(target_os = "windows"))]
    let mut copy = std::process::Command::new("cp");
    #[cfg(target_os = "windows")]
    let mut copy = std::process::Command::new("copy");

    if uefi {
        #[cfg(not(target_os = "windows"))]
        copy.arg(up).arg("./target/uefi.img");
        #[cfg(target_os = "windows")]
        copy.arg(up).arg(".\\target\\uefi.img");
    } else {
        #[cfg(not(target_os = "windows"))]
        copy.arg(bp).arg("./target/bios.img");
        #[cfg(target_os = "windows")]
        copy.arg(bp).arg(".\\target\\bios.img");
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

    #[cfg(target_os = "linux")]
    cmd.arg("-enable-kvm");

    cmd.arg("-m").arg("4G");
    cmd.arg("-serial").arg("stdio");
    cmd.arg("-device").arg("piix3-ide,id=ide");
    cmd.arg("-drive")
        .arg("id=disk,file=hd.img,format=raw,if=none");
    cmd.arg("-device").arg("ide-hd,drive=disk,bus=ide.0");
    cmd.arg("-device").arg("vmware-svga");
    cmd.arg("-machine").arg("q35");
    cmd.arg("-cpu").arg("host");

    let mut child = cmd.spawn().unwrap();
    child.wait().unwrap();
}
