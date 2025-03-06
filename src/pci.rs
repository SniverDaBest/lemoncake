use crate::{info, inl, outl};
use alloc::vec::*;
use core::fmt::{Formatter, Display, self};

pub const PCI_CONFIG_ADDR: u16 = 0xCF8;
pub const PCI_CONFIG_DATA: u16 = 0xCFC;

pub struct PCIDevice {
    pub bus: u8,
    pub device: u8,
    pub func: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub rev: u8,
    pub prog_if: u8,
    pub subclass: u8,
    pub class_id: u8,
}

impl PCIDevice {
    pub fn new(bus: u8, device: u8, func: u8, vendor_id: u16, device_id: u16, rev: u8, prog_if: u8, subclass: u8, class_id: u8) -> Self {
        Self { bus, device, func, vendor_id, device_id, rev, prog_if, class_id, subclass }
    }
}

impl Display for PCIDevice {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Bus: {} | Device: {} | Function: {} | Vendor ID: {} | Device ID: {} | Revision: {} | Programming Interface: {} | Subclass: {} | Class ID: {}", self.bus, self.device, self.func, self.vendor_id, self.device_id, self.rev, self.prog_if, self.subclass, self.class_id)
    }
}

pub unsafe fn pci_config_read(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let addr: u32 =
        (1 << 31)
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);
    
    unsafe {
        outl(PCI_CONFIG_ADDR, addr);
        return inl(PCI_CONFIG_DATA);
    }
}

pub unsafe fn pci_config_write(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    let address: u32 =
        (1 << 31)
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC);
    
    unsafe {
        outl(PCI_CONFIG_ADDR, address);
        outl(PCI_CONFIG_DATA, value);
    }
}

pub unsafe fn scan_pci_bus() -> Vec<PCIDevice> {
    let mut res: Vec<PCIDevice> = Vec::new();
    for bus in 0..=255 {
        for device in 0..32 {
            for function in 0..8 {
                unsafe {
                    let vendor_id = pci_config_read(bus, device, function, 0) as u16;

                    if vendor_id == 0xFFFF {
                        continue;
                    }

                    let device_id = (pci_config_read(bus, device, function, 0) >> 16) as u16;

                    let reg = pci_config_read(bus, device, function, 0x08);
                    let rev = (reg & 0xFF) as u8;
                    let prog_if = ((reg >> 8) & 0xFF) as u8;
                    let subclass = ((reg >> 16) & 0xFF) as u8;
                    let class = (reg >> 24) as u8;
                
                    res.push(PCIDevice::new(bus, device, function, vendor_id, device_id, rev, prog_if, subclass, class));
                }
            }
        }
    }
    return res;
}