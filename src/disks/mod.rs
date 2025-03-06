// SATA
pub const SATA_SIG_ATA: u32 = 0x00000101;
pub const SATA_SIG_ATAPI: u32 = 0xEB140101;
pub const SATA_SIG_SEMB: u32 = 0xC33C0101;
pub const SATA_SIG_PM: u32 = 0x96690101;

// AHCI
pub const AHCI_BASE: u32 = 0x400000;
pub const AHCI_DEV_NULL: u8 = 0;
pub const AHCI_DEV_SATA: u8 = 1;
pub const AHCI_DEV_SEMB: u8 = 2;
pub const AHCI_DEV_PM: u8 = 3;
pub const AHCI_DEV_SATAPI: u8 = 4;

// HBA
pub const HBA_PORT_IPM_ACTIVE: u8 = 1;
pub const HBA_PORT_DET_PRESENT: u8 = 3;
pub const HBA_PXCMD_ST: u32 = 0x0001;
pub const HBA_PXCMD_FRE: u32 = 0x0010;
pub const HBA_PXCMD_FR: u32 = 0x4000;
pub const HBA_PXCMD_CR: u32 = 0x8000;
pub const HBA_PXIS_TFES: u32 = 0x40000000;

// ATA
pub const ATA_DEV_BUSY: u8 = 0x80;
pub const ATA_DEV_DRQ: u8 = 0x08;
pub const ATA_CMD_READ_DMA_EX: u8 = 0x25;

// Misc.
pub const CMD_SLOTS: usize = 32;

pub mod ahci;
