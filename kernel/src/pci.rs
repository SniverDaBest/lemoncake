use crate::error;
use alloc::vec::*;
use core::{arch::asm, fmt};

/// The PCI Device type.
#[derive(Debug, Copy, Clone)]
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
    ) -> Self {
        return Self {
            vendor_id,
            device_id,
            class_code,
            subclass,
            bus,
            slot,
            func,
        };
    }

    pub unsafe fn write_pci(&self, offset: u8, value: u32) {
        let address = (1 << 31)
            | ((self.bus as u32) << 16)
            | ((self.slot as u32) << 11)
            | ((self.func as u32) << 8)
            | ((offset as u32) & 0xFC);

        unsafe {
            asm!("out dx, eax", in("dx") 0xCF8, in("eax") address);
            asm!("out dx, eax", in("dx") 0xCFC, in("eax") value);
        }
    }

    pub unsafe fn read_pci(&self, offset: u8) -> u32 {
        let address = (1 << 31)
            | ((self.bus as u32) << 16)
            | ((self.slot as u32) << 11)
            | ((self.func as u32) << 8)
            | ((offset as u32) & 0xFC);

        unsafe {
            asm!("out dx, eax", in("dx") 0xCF8, in("eax") address);
            let mut data: u32;
            asm!("in eax, dx", in("dx") 0xCFC, out("eax") data);
            return data;
        }
    }

    pub unsafe fn read_bar(&self, bar: u32) -> Option<u32> {
        if bar > 5 {
            error!("(PCI) Bar no. {} is greater than 5!", bar);
            return None;
        } else {
            return Some(self.read_pci_config((0x10 + bar * 4) as u8));
        }
    }

    pub unsafe fn read_pci_config(&self, offset: u8) -> u32 {
        let addr = (1 << 31)
            | ((self.bus as u32) << 16)
            | ((self.slot as u32) << 11)
            | ((self.func as u32) << 8)
            | ((offset as u32) & 0xFC);
        asm!("out dx, eax", in("dx") 0xCF8, in("eax") addr);
        let mut value: u32;
        asm!("in eax, dx", out("eax") value, in("dx") 0xCFC);
        return value;
    }

    pub unsafe fn write_pci_config(&self, offset: u8, value: u32) {
        let address = (1 << 31)
            | ((self.bus as u32) << 16)
            | ((self.slot as u32) << 11)
            | ((self.func as u32) << 8)
            | ((offset as u32) & 0xFC);
        asm!("out dx, eax", in("dx") 0xCF8, in("eax") address);
        asm!("out dx, eax", in("dx") 0xCFC, in("eax") value);
    }

    pub fn from_bsf(bus: u8, slot: u8, func: u8) -> Option<Self> {
        let dummy = Self::new(0, 0, 0, 0, bus, slot, func);
        let vendor_id = unsafe { Self::read_pci_config(&dummy, 0x00) } & 0xFFFF;
        if vendor_id == 0xFFFF {
            return None;
        }
        let device_id = unsafe { Self::read_pci_config(&dummy, 0x00) >> 16 };
        let class_code = unsafe { Self::read_pci_config(&dummy, 0x08) >> 24 };
        let subclass = (unsafe { Self::read_pci_config(&dummy, 0x08) } >> 16) & 0xFF;

        return Some(Self::new(
            vendor_id, device_id, class_code, subclass, bus, slot, func,
        ));
    }

    pub fn prog_if(&self) -> u8 {
        return unsafe { ((self.read_pci_config(0x08) >> 8) & 0xFF) as u8 };
    }
}

impl fmt::Display for PCIDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Vendor ID: {:#x} | Device ID: {:#x} | Class code: {:#x} | Subclass: {:#x} | Bus: {} | Slot: {} | Func: {:#x}",
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

pub fn scan_pci_bus() -> Vec<PCIDevice> {
    let mut devices = Vec::new();
    for bus in 0..255 {
        for slot in 0..32 {
            for func in 0..8 {
                let d = PCIDevice::from_bsf(bus, slot, func);
                if d.is_some() {
                    devices.push(d.unwrap());
                }
            }
        }
    }
    return devices;
}
