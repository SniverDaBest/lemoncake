use core::fmt::{self, Formatter, Display};
use crate::vga;
use multiboot2::{BootInformation, RsdpV1Tag, RsdpV2Tag};
use alloc::vec::*;

pub struct RSDP {
    v1: Option<RsdpV1Tag>,
    v2: Option<RsdpV2Tag>,
}

impl RSDP {
    pub fn new(v1: Option<&RsdpV1Tag>, v2: Option<&RsdpV2Tag>) -> Self {
        if v2.is_some() {
            RSDP {
                v1: None,
                v2: Some(*v2.unwrap()),
            }
        } else {
            if v1.is_some() {
                RSDP {
                    v1: Some(*v1.unwrap()),
                    v2: None,
                }
            } else {
                panic!("Couldn't get any RSDP! Is there ACPI on this machine?");
            }
        }
    }

    pub fn is_checksum_valid(&self) -> bool {
        if self.v2.is_some() {
            self.v2.unwrap().checksum_is_valid()
        } else { // we don't need to check if there is no rsdp; we already checked.
            self.v1.unwrap().checksum_is_valid()
        }
    }

    pub fn get_signature(&self) -> &str {
        if let Some(v2) = &self.v2 {
            v2.signature().unwrap()
        } else if let Some(v1) = &self.v1 {
            v1.signature().unwrap()
        } else {
            panic!("No RSDP found");
        }
    }

    pub fn get_revision(&self) -> u8 {
        if self.v2.is_some() {
            self.v2.unwrap().revision()
        } else { // we don't need to check if there is no rsdp; we already checked.
            self.v1.unwrap().revision()
        }
    }

    /// Gets RSDT for v1 and XSDT for v2.
    pub fn get_rsdt_addr(&self) -> usize {
        if self.v2.is_some() {
            self.v2.unwrap().xsdt_address()
        } else {
            self.v1.unwrap().rsdt_address()
        }
    }

    pub fn get_oem_id(&self) -> &str {
        if let Some(v2) = &self.v2 {
            v2.oem_id().unwrap()
        } else if let Some(v1) = &self.v1 {
            v1.oem_id().unwrap()
        } else {
            panic!("No RSDP found!");
        }
    }

    /// This is ONLY XSDP (v2)! Not RSDP (v1).
    pub fn get_ext_checksum(&self) -> Result<u8, &str>{
        if self.v2.is_none() {
            return Err("You're using RSDP, not XSDP!")
        } else {
            Ok(self.v2.unwrap().ext_checksum())
        }
    }
}

impl Display for RSDP {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Is XSDP: {} | OEM ID: {} | Revision: {} | RSDT Address: {:#X?} | Signature: {} | Is Checksum Valid: {} | Extended Checksum: {:?}", self.v2.is_some(), self.get_oem_id(), self.get_revision(), self.get_rsdt_addr(), self.get_signature(), self.is_checksum_valid(), self.get_ext_checksum())
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct SDTHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}
