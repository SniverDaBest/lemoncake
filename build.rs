use std::{path::PathBuf,env::var_os};
use bootloader::{BiosBoot,UefiBoot};

fn main() {
    let od = PathBuf::from(var_os("OUT_DIR").unwrap());
    let kernel = PathBuf::from(var_os("CARGO_BIN_FILE_KERNEL_kernel").unwrap());
    
    let uefi_path = od.join("uefi.img");
    let bios_path = od.join("bios.img");

    UefiBoot::new(&kernel).create_disk_image(uefi_path.as_path()).unwrap();
    BiosBoot::new(&kernel).create_disk_image(bios_path.as_path()).unwrap();

    println!("cargo:rustc-env=UEFI_PATH={}", uefi_path.display());
    println!("cargo:rustc-env=BIOS_PATH={}", bios_path.display());
}
