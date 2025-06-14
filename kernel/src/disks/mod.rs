pub mod ahci;
// TODO: IDE (as you can see, i've already gotten the structs done)

const SATA_SIG_ATA: u32 = 0x00000101;
const SATA_SIG_ATAPI: u32 = 0xEB140101;
const SATA_SIG_SEMB: u32 = 0xC33C0101;
const SATA_SIG_PM: u32 = 0x96690101;

const AHCI_BASE: u32 = 0x400000;

const AHCI_DEV_NULL: u32 = 0;
const AHCI_DEV_SATA: u32 = 1;
const AHCI_DEV_SEMB: u32 = 2;
const AHCI_DEV_PM: u32 = 3;
const AHCI_DEV_SATAPI: u32 = 4;

const HBA_PORT_IPM_ACTIVE: u8 = 1;
const HBA_PORT_DET_PRESENT: u8 = 3;

const HBA_PXCMD_ST: u32 = 0x0001;
const HBA_PXCMD_FRE: u32 = 0x0010;
const HBA_PXCMD_FR: u32 = 0x4000;
const HBA_PXCMD_CR: u32 = 0x8000;
const HBA_PXIS_TFES: u8 = 0x01;

const ATA_DEV_BUSY: u8 = 0x80;
const ATA_DEV_DRQ: u8 = 0x08;
const ATA_DEV_DRDY: u8 = 0x40;
const ATA_DEV_DF: u8 = 0x20;
const ATA_DEV_DSC: u8 = 0x10;
const ATA_DEV_CORR: u8 = 0x04;
const ATA_DEV_IDX: u8 = 0x02;
const ATA_DEV_ERR: u8 = 0x01;
const ATA_ERR_BBK: u8 = 0x80;
const ATA_ERR_UNC: u8 = 0x40;
const ATA_ERR_MC: u8 = 0x20;
const ATA_ERR_IDNF: u8 = 0x10;
const ATA_ERR_MCR: u8 = 0x08;
const ATA_ERR_ABRT: u8 = 0x04;
const ATA_ERR_TK0NF: u8 = 0x02;
const ATA_ERR_AMFN: u8 = 0x01;

const ATA_CMD_READ_PIO: u8 = 0x20;
const ATA_CMD_READ_PIO_EX: u8 = 0x24;
const ATA_CMD_READ_DMA: u8 = 0xC8;
const ATA_CMD_READ_DMA_EX: u8 = 0x25;
const ATA_CMD_WRITE_PIO: u8 = 0x30;
const ATA_CMD_WRITE_PIO_EX: u8 = 0x34;
const ATA_CMD_WRITE_DMA: u8 = 0xCA;
const ATA_CMD_WRITE_DMA_EX: u8 = 0x35;
const ATA_CMD_CACHE_FLUSH: u8 = 0xE7;
const ATA_CMD_CACHE_FLUSH_EX: u8 = 0xEA;
const ATA_CMD_PACKET: u8 = 0xA0;
const ATA_CMD_IDENTIFY_PACKET: u8 = 0xA1;
const ATA_CMD_IDENTIFY: u8 = 0xEC;

const ATA_IDENT_DEVICETYPE: u8 = 0;
const ATA_IDENT_CYLINDERS: u8 = 2;
const ATA_IDENT_HEADS: u8 = 6;
const ATA_IDENT_SECTORS: u8 = 12;
const ATA_IDENT_SERIAL: u8 = 20;
const ATA_IDENT_MODEL: u8 = 54;
const ATA_IDENT_CAPABILITIES: u8 = 98;
const ATA_IDENT_FIELDVALID: u8 = 106;
const ATA_IDENT_MAX_LBA: u8 = 120;
const ATA_IDENT_COMMANDSETS: u8 = 164;
const ATA_IDENT_MAX_LBA_EX: u8 = 200;

const ATA_PARENT: u8 = 0x00;
const ATA_CHILD: u8 = 0x01;

const ATA_PRIMARY: u8 = 0x00;
const ATA_SECONDARY: u8 = 0x01;

const ATA_REG_DATA: u8 = 0x00;
const ATA_REG_ERR: u8 = 0x01;
const ATA_REG_FEATS: u8 = 0x01;
const ATA_REG_SECCNT0: u8 = 0x02;
const ATA_REG_LBA0: u8 = 0x03;
const ATA_REG_LBA1: u8 = 0x04;
const ATA_REG_LBA2: u8 = 0x05;
const ATA_REG_HDDEVSEL: u8 = 0x06;
const ATA_REG_CMD: u8 = 0x07;
const ATA_REG_STAT: u8 = 0x07;
const ATA_REG_SECCNT1: u8 = 0x08;
const ATA_REG_LBA3: u8 = 0x09;
const ATA_REG_LBA4: u8 = 0x0A;
const ATA_REG_LBA5: u8 = 0x0B;
const ATA_REG_CTRL: u8 = 0x0C;
const ATA_REG_ALTSTAT: u8 = 0x0C;
const ATA_REG_DEVADDR: u8 = 0x0D;

const ATAPI_CMD_READ: u8 = 0xAB;
const ATAPI_CMD_EJECT: u8 = 0x1B;
