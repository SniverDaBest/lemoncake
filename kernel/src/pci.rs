use crate::error;
use alloc::vec::*;
use core::{
    arch::asm,
    fmt,
    ptr::{read_volatile, write_volatile},
};

const PCI_CMD_OFFSET: u16 = 0x04;
const PCI_CMD_BUS_MASTER: u16 = 1 << 2;
const PCI_CMD_MEMORY_ENABLE: u16 = 1 << 0;
static mut ECAM_BASE: usize = 0;

pub unsafe fn set_ecam_base(base: usize) {
    ECAM_BASE = base;
}

pub fn ecam_read32(bus: u8, slot: u8, func: u8, offset: u16) -> Option<u32> {
    let base = unsafe { crate::pci::ECAM_BASE };
    if base == 0 {
        return None;
    }

    let addr = (base as *const u32).wrapping_add(
        ((bus as usize) << 20)
            + ((slot as usize) << 15)
            + ((func as usize) << 12)
            + (offset as usize / 4),
    );
    return Some(unsafe { core::ptr::read_volatile(addr) });
}

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
    pub fn new(
        vendor_id: u32,
        device_id: u32,
        class_code: u32,
        subclass: u32,
        bus: u8,
        slot: u8,
        func: u8,
    ) -> Self {
        Self {
            vendor_id,
            device_id,
            class_code,
            subclass,
            bus,
            slot,
            func,
        }
    }

    pub unsafe fn read_config(&self, offset: u16) -> u32 {
        let ecam = ECAM_BASE;
        if ecam != 0 {
            let addr = (ecam
                + ((self.bus as usize) << 20)
                + ((self.slot as usize) << 15)
                + ((self.func as usize) << 12)
                + (offset as usize)) as *const u32;

            return read_volatile(addr);
        } else {
            let address = (1u32 << 31)
                | ((self.bus as u32) << 16)
                | ((self.slot as u32) << 11)
                | ((self.func as u32) << 8)
                | ((offset as u32) & 0xFC);

            let mut data: u32;
            asm!("out dx, eax", in("dx") 0xCF8u16, in("eax") address);
            asm!("in eax, dx", out("eax") data, in("dx") 0xCFCu16);
            return data;
        }
    }

    pub unsafe fn write_config(&self, offset: u16, value: u32) {
        let ecam = ECAM_BASE;
        if ecam != 0 {
            let addr = (ecam
                + ((self.bus as usize) << 20)
                + ((self.slot as usize) << 15)
                + ((self.func as usize) << 12)
                + (offset as usize)) as *mut u32;
            write_volatile(addr, value);
        } else {
            let address = (1u32 << 31)
                | ((self.bus as u32) << 16)
                | ((self.slot as u32) << 11)
                | ((self.func as u32) << 8)
                | ((offset as u32) & 0xFC);

            asm!("out dx, eax", in("dx") 0xCF8u16, in("eax") address);
            asm!("out dx, eax", in("dx") 0xCFCu16, in("eax") value);
        }
    }

    pub fn read_config32(bus: u8, slot: u8, func: u8, offset: u16) -> u32 {
        if let Some(val) = ecam_read32(bus, slot, func, offset) {
            return val;
        }

        let address = (1u32 << 31)
            | ((bus as u32) << 16)
            | ((slot as u32) << 11)
            | ((func as u32) << 8)
            | ((offset as u32) & 0xFC);
        unsafe {
            core::arch::asm!("out dx, eax", in("dx") 0xCF8, in("eax") address);
            let mut data: u32;
            core::arch::asm!("in eax, dx", out("eax") data, in("dx") 0xCFC);
            return data;
        }
    }

    pub unsafe fn read_config_u16(&self, offset: u16) -> u16 {
        return (self.read_config(offset & !0x3) >> ((offset & 3) * 8)) as u16;
    }

    pub unsafe fn read_config_u8(&self, offset: u16) -> u8 {
        return (self.read_config(offset & !0x3) >> ((offset & 3) * 8)) as u8;
    }

    pub unsafe fn write_config_u16(&self, offset: u16, value: u16) {
        let aligned = offset & !0x3;
        let shift = ((offset & 3) * 8) as u32;
        let mut old = self.read_config(aligned);
        let mask = !(0xFFFFu32 << shift);
        old = (old & mask) | ((value as u32) << shift);
        self.write_config(aligned, old);
    }

    pub unsafe fn write_config_u8(&self, offset: u16, value: u8) {
        let aligned = offset & !0x3;
        let shift = ((offset & 3) * 8) as u32;
        let mut old = self.read_config(aligned);
        let mask = !(0xFFu32 << shift);
        old = (old & mask) | ((value as u32) << shift);
        self.write_config(aligned, old);
    }

    pub unsafe fn read_bar(&self, bar: u8) -> Option<(u32, bool, bool)> {
        if bar > 5 {
            error!("(PCI) Bar no. {} is greater than 5!", bar);
            return None;
        }
        let offset = 0x10 + (bar as u16) * 4;
        let low = self.read_config(offset);
        return Some((
            low,
            (low & 0x1) == 0,
            (low & 0x1) == 0 && ((low >> 1) & 0x3 == 0x2),
        ));
    }

    pub unsafe fn bar_address(&self, bar: u8) -> Option<u64> {
        let (low, is_mem, is_64) = self.read_bar(bar)?;
        if !is_mem {
            let addr = (low & 0xFFFFFFFC) as u64;
            return Some(addr);
        } else if is_64 {
            let low_masked = low & 0xFFFF_FFF0;
            let high = self.read_config(0x10 + (bar as u16) * 4 + 4);
            let addr = ((high as u64) << 32) | (low_masked as u64);
            return Some(addr);
        } else {
            let addr = (low & 0xFFFF_FFF0) as u64;
            return Some(addr);
        }
    }

    pub unsafe fn probe_bar_size(&self, bar: u8) -> Option<u64> {
        if bar > 5 {
            return None;
        }
        let offset = 0x10 + (bar as u16) * 4;

        let orig_low = self.read_config(offset);

        self.write_config(offset, 0xFFFF_FFFF);
        let masked_low = self.read_config(offset);

        self.write_config(offset, orig_low);

        let (_orig_low_saved, is_mem, is_64) = self.read_bar(bar)?;

        if !is_mem {
            let mask = masked_low & 0xFFFF_FFFC;
            if mask == 0 {
                return None;
            }
            let size = (!(mask) + 1) as u64;
            return Some(size);
        } else if is_64 {
            let orig_high = self.read_config(offset + 4);
            self.write_config(offset + 4, 0xFFFF_FFFF);
            let masked_high = self.read_config(offset + 4);

            self.write_config(offset + 4, orig_high);

            let full_mask = ((masked_high as u64) << 32) | ((masked_low & 0xFFFF_FFF0) as u64);
            if full_mask == 0 {
                return None;
            }
            let size = !(full_mask) + 1;
            return Some(size);
        } else {
            let mask = masked_low & 0xFFFF_FFF0;
            if mask == 0 {
                return None;
            }
            let size = (!(mask) + 1) as u64;
            return Some(size);
        }
    }

    pub unsafe fn read_pci(&self, offset: u8) -> u32 {
        return self.read_config(offset as u16);
    }

    pub unsafe fn write_pci(&self, offset: u8, value: u32) {
        return self.write_config(offset as u16, value);
    }

    pub unsafe fn read_pci_config(&self, offset: u8) -> u32 {
        return self.read_config(offset as u16);
    }

    pub unsafe fn write_pci_config(&self, offset: u8, value: u32) {
        return self.write_config(offset as u16, value);
    }

    pub fn from_bsf(bus: u8, slot: u8, func: u8) -> Option<Self> {
        let vendor_device = Self::read_config32(bus, slot, func, 0x0);
        let vendor_id = (vendor_device & 0xFFFF) as u16;
        if vendor_id == 0xFFFF {
            return None;
        }
        let device_id = ((vendor_device >> 16) & 0xFFFF) as u16;
        let class_reg = Self::read_config32(bus, slot, func, 0x8);
        let class_code = (class_reg >> 24) & 0xFF;
        let subclass = (class_reg >> 16) & 0xFF;
        return Some(Self::new(
            vendor_id as u32,
            device_id as u32,
            class_code,
            subclass,
            bus,
            slot,
            func,
        ));
    }

    pub fn prog_if(&self) -> u8 {
        return unsafe { ((self.read_config(0x08) >> 8) & 0xFF) as u8 };
    }

    pub unsafe fn enable_bus_master(&self) -> bool {
        let mut cmd = self.read_config_u16(PCI_CMD_OFFSET);

        cmd |= PCI_CMD_MEMORY_ENABLE;
        cmd |= PCI_CMD_BUS_MASTER;

        self.write_config_u16(PCI_CMD_OFFSET, cmd);

        let verify = self.read_config_u16(PCI_CMD_OFFSET);
        if verify & PCI_CMD_BUS_MASTER != 0 {
            return true;
        }

        return false;
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
    for bus in 0..=255 {
        for slot in 0..32 {
            for func in 0..8 {
                if let Some(dev) = PCIDevice::from_bsf(bus, slot, func) {
                    devices.push(dev);

                    if func == 0 && (PCIDevice::read_config32(bus, slot, 0, 0x0) & (1 << 7)) == 0 {
                        break;
                    }
                }
            }
        }
    }
    return devices;
}
