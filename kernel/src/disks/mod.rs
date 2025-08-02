//pub mod ahci;
pub mod nvme;

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
const ATA_CMD_READ_DMA_EX: u8 = 0x25;
const ATA_CMD_WRITE_DMA_EX: u8 = 0x35;

const NVME_PCI_CLASS: u32 = 0x01;
const NVME_PCI_SUBCLASS: u32 = 0x08;

const NVME_PCI_PROG_IF: u32 = 0x02;

const NVME_REG_CAP: u64 = 0x00;
const NVME_REG_VS: u64 = 0x08;
const NVME_REG_CC: u64 = 0x14;
const NVME_REG_CSTS: u64 = 0x1C;
const NVME_REG_AQA: u64 = 0x24;
const NVME_REG_ASQ: u64 = 0x28;
const NVME_REG_ACQ: u64 = 0x30;

const NVME_CAP_MQES_MASK: u64 = 0xFFFF;
const NVME_CAP_CQR: u64 = 1 << 16;
const NVME_CAP_DSTRD_MASK: u64 = 0xF << 32;
const NVME_CAP_CSS_MASK: u64 = 0xFF << 37;
const NVME_CAP_MPSMIN_MASK: u64 = 0xF << 48;
const NVME_CAP_MPSMAX_MASK: u64 = 0xF << 52;

const NVME_CC_EN: u32 = 1 << 0;
const NVME_CC_CSS_NVM: u32 = 0 << 4;
const NVME_CC_SHN_NORMAL: u32 = 1 << 14;

const NVME_CSTS_RDY: u32 = 1 << 0;
const NVME_CSTS_CFS: u32 = 1 << 1;

const NVME_MAX_IO_QUEUES: u32 = 16;

const NVME_ADMIN_QUEUE_SZ: u16 = 16;

const NVME_ADMIN_DELETE_SQ: u8 = 0x00;
const NVME_ADMIN_CREATE_SQ: u8 = 0x01;
const NVME_ADMIN_DELETE_CQ: u8 = 0x04;
const NVME_ADMIN_CREATE_CQ: u8 = 0x05;
const NVME_ADMIN_IDENT: u8 = 0x06;
const NVME_ADMIN_GET_FEATS: u8 = 0x0A;

const NVME_TIMEOUT_MS: u64 = 15000;

const NVME_SC_SUCCESS: u16 = 0x00;

const NVME_IO_FLUSH: u8 = 0x00;
const NVME_IO_WRITE: u8 = 0x01;
const NVME_IO_READ: u8 = 0x02;
const NVME_IO_WRITE_ZEROS: u8 = 0x08;
const NVME_IO_COMPARE: u8 = 0x05;
