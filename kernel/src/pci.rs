use alloc::vec::*;
use core::{arch::asm, fmt};

/// The PCI Device type.
#[derive(Debug)]
pub struct PCIDevice {
    pub vendor_id: u32,
    pub device_id: u32,
    pub class_code: u32,
    pub subclass: u32,

    pub bus: u8,
    pub slot: u8,
    pub func: u8,
}

impl PCIDevice {
    /// Creates a new PCI Device type.
    pub fn new(
        vendor_id: u32,
        device_id: u32,
        class_code: u32,
        subclass: u32,
        bus: u8,
        slot: u8,
        func: u8,
    ) -> PCIDevice {
        return PCIDevice {
            vendor_id: vendor_id,
            device_id: device_id,
            class_code: class_code,
            subclass: subclass,
            bus: bus,
            slot: slot,
            func: func,
        };
    }
}

impl fmt::Display for PCIDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Vendor ID: {} | Device ID: {} | Class code: {} | Subclass: {} | Bus: {} | Slot: {} | Func: {}",
            self.vendor_id,
            self.device_id,
            self.class_code,
            self.subclass,
            self.bus,
            self.slot,
            self.func
        )
    }
}

/// Writes to a certain PCI device, at a certain offset.
pub fn write_pci(offset: u8, pci_device: &PCIDevice, value: u32) {
    let address = (1 << 31)
        | ((pci_device.bus as u32) << 16)
        | ((pci_device.slot as u32) << 11)  // Slot should be correct
        | ((pci_device.func as u32) << 8)
        | ((offset as u32) & 0xFC);

    unsafe {
        // Write to PCI address port
        core::arch::asm!("out dx, eax", in("dx") 0xCF8, in("eax") address);

        // Write to PCI data port
        core::arch::asm!("out dx, eax", in("dx") 0xCFC, in("eax") value);
    }
}

/// Reads from a certain PCI device, at a certain offset.
pub fn read_pci(offset: u8, pci_device: &PCIDevice) -> u32 {
    let address = (1 << 31)
        | ((pci_device.bus as u32) << 16)
        | ((pci_device.slot as u32) << 11)  // This should be slot instead of device_id
        | ((pci_device.func as u32) << 8)
        | ((offset as u32) & 0xFC);

    unsafe {
        // Write to PCI address port
        core::arch::asm!("out dx, eax", in("dx") 0xCF8, in("eax") address);

        // Read from PCI data port
        let data: u32;
        core::arch::asm!("in eax, dx", in("dx") 0xCFC, out("eax") data);
        data
    }
}

/// Reads from the PCI config space via the IO ports.
pub unsafe fn read_pci_config(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    let addr = (1 << 31)
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);
    asm!("out dx, eax", in("dx") 0xCF8, in("eax") addr);
    let value: u32;
    asm!("in eax, dx", out("eax") value, in("dx") 0xCFC);
    return value;
}

/// Writes to the PCI config space via the IO ports.
pub unsafe fn write_pci_config(bus: u8, slot: u8, func: u8, offset: u8, value: u32) {
    let address = (1 << 31)
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);
    asm!("out dx, eax", in("dx") 0xCF8, in("eax") address);
    asm!("out dx, eax", in("dx") 0xCFC, in("eax") value);
}

/// Scans the PCI bus. Returns a Vec<PCIDevice>
pub fn scan_pci_bus() -> Vec<PCIDevice> {
    let mut devices: Vec<PCIDevice> = Vec::new();
    for bus in 0..255 {
        for slot in 0..32 {
            for func in 0..8 {
                let vendor_id = unsafe { read_pci_config(bus, slot, func, 0x00) } & 0xFFFF;
                if vendor_id != 0xFFFF {
                    let device_id = unsafe { read_pci_config(bus, slot, func, 0x00) >> 16 };
                    let class_code = unsafe { read_pci_config(bus, slot, func, 0x08) >> 24 };
                    let subclass = (unsafe { read_pci_config(bus, slot, func, 0x08) } >> 16) & 0xFF;

                    devices.push(PCIDevice::new(
                        vendor_id, device_id, class_code, subclass, bus, slot, func,
                    ));
                }
            }
        }
    }

    return devices;
}
