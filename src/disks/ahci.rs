use core::{fmt::{self, Display, Formatter}, ptr};
use crate::{bool_to_yn, error, info, pci::{scan_pci_bus, PCIDevice}, warning};

use super::*;
use alloc::vec::Vec;
use bitfield_struct::bitfield;

#[repr(C)]
pub enum FISType {
	FISTypeRegH2D	= 0x27,	// Register FIS - host to device
	FISTypeRegD2H	= 0x34,	// Register FIS - device to host
	FISTypeDMAACT	= 0x39,	// DMA activate FIS - device to host
	FISTypeDMASetup	= 0x41,	// DMA setup FIS - bidirectional
	FISTypeData		= 0x46,	// Data FIS - bidirectional
	FisTypeBist		= 0x58,	// BIST activate FIS - bidirectional
	FISTypePIOSetup	= 0x5F,	// PIO setup FIS - device to host
	FISTypeDevBits	= 0xA1,	// Set device bits FIS - device to host
}

#[bitfield(u8)]
pub struct FisRegH2DFlags {
    #[bits(4)]
    pub pmport: u8,
    #[bits(3)]
    pub rsv0: u8,
    pub c: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FISRegH2D {
    pub fis_type: u8,
    pub flags: FisRegH2DFlags,
    pub command: u8,
    pub featurel: u8,
    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,
    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    pub featureh: u8,
    pub countl: u8,
    pub counth: u8,
    pub icc: u8,
    pub control: u8,
    pub rsv1: [u8; 4],
}

#[bitfield(u8)]
pub struct FisRegD2HFlags {
    #[bits(4)]
    pub pmport: u8,
    #[bits(2)]
    pub rsv0: u8,
    pub i: bool,
    pub rsv1: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FISRegD2H {
    pub fis_type: u8,
    pub flags: FisRegD2HFlags, // pmport, rsv0, i, rsv1
    pub status: u8,
    pub error: u8,
    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,
    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    pub rsv2: u8,
    pub countl: u8,
    pub counth: u8,
    pub rsv3: [u8; 2],
    pub rsv4: [u8; 4],
}

#[bitfield(u8)]
pub struct FISDMASetupFlags {
    #[bits(4)]
    pub pmport: u8,
    pub rsv0: bool,
    pub d: bool,
    pub i: bool,
    pub a: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FISDMASetup {
    pub fis_type: u8,
    pub flags: FISDMASetupFlags,
    pub rsved: [u8; 2],
    pub DMAbufferID: u64,
    pub rsvd: u32,
    pub DMAbufOffset: u32,
    pub TransferCount: u32,
    pub resvd: u32,
}


#[bitfield(u8)]
pub struct FisDataFlags {
    #[bits(4)]
    pub pmport: u8,
    #[bits(4)]
    pub rsv0: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FISData {
    pub fis_type: u8,
    pub flags: FisDataFlags,
    pub rsv1: [u8; 2],
    pub data: [u32; 1],
}

#[bitfield(u8)]
pub struct FISPIOSetupFlags {
    #[bits(4)]
    pub pmport: u8,
    pub rsv0: bool,
    pub d: bool,
    pub i: bool,
    pub rsv1: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FISPIOSetup {
    pub fis_type: u8,
    pub flags: FISPIOSetupFlags,
    pub status: u8,
    pub error: u8,
    pub lba0: u8,
    pub lba1: u8,
    pub lba2: u8,
    pub device: u8,
    pub lba3: u8,
    pub lba4: u8,
    pub lba5: u8,
    pub rsv2: u8,
    pub countl: u8,
    pub counth: u8,
    pub rsv3: u8,
    pub e_status: u8,
    pub tc: u16,
    pub rsv4: [u8; 2],
}
#[repr(C)]
pub struct FISDevBits;

/// THIS STRUCT IS VOLATILE!
#[repr(C)]
pub struct HBAPort {
    clb: u32,
    clbu: u32,
    fb: u32,
    fbu: u32,
    is: u32,
    ie: u32,
    cmd: u32,
    rsv0: u32,
    tfd: u32,
    sig: u32,
    ssts: u32,
    sctl: u32,
    serr: u32,
    sact: u32,
    ci: u32,
    sntf: u32,
    fbs: u32,
    rsv1: [u32; 11],
    vendor: [u32; 4],
}

/// THIS STRUCT IS VOLATILE!
#[repr(C)]
pub struct HBAMem {
    cap: u32,
    ghc: u32,
    is: u32,
    pi: u32,
    vs: u32,
    ccc_ctl: u32,
    ccc_pts: u32,
    em_loc: u32,
    em_ctl: u32,
    cap2: u32,
    bohc: u32,

    rsv: [u8; 0xA0-0x2C],

    vendor: [u8; 0x100-0xA0],

    ports: [HBAPort; 1],
}

/// THIS STRUCT IS VOLATILE!
#[repr(C)]
pub struct HBAFIS {
    dsfis: FISDMASetup,
    pad0: [u8; 4],

    psfis: FISPIOSetup,
    pad1: [u8; 12],

    rfis: FISRegD2H,
    pad2: [u8; 4],

    sdbfis: FISDevBits,
    
    ufis: [u8; 64],

    rsv: [u8; 0x100-0xA0]
}

#[bitfield(u8)]
pub struct HBACMDHeaderByte0 {
    #[bits(5)]
    pub cfl: u8,
    pub a: bool,
    pub w: bool,
    pub p: bool,
}

#[bitfield(u8)]
pub struct HBACMDHeaderByte1 {
    pub r: bool,
    pub b: bool,
    pub c: bool,
    pub rsv0: bool,
    #[bits(4)]
    pub pmp: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HBACMDHeader {
    pub byte0: HBACMDHeaderByte0,
    pub byte1: HBACMDHeaderByte1,
    pub prdtl: u16,
    // Note: Rust does not have a 'volatile' type by default.
    pub prdbc: u32,
    pub ctba: u32,
    pub ctbau: u32,
    pub rsv1: [u32; 4],
}

#[bitfield(u32)]
pub struct HBAPRDTEntryFlags {
    #[bits(22)]
    pub dbc: u32,
    #[bits(9)]
    pub rsv1: u16,
    pub i: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HBAPRDTEntry {
    pub dba: u32,
    pub dbau: u32,
    pub rsv0: u32,
    pub flags: HBAPRDTEntryFlags,
}

impl HBAPRDTEntry {
    pub fn set_dbc(&mut self, value: u32) {
        self.flags.set_dbc(value);
    }

    pub fn set_i(&mut self, flag: bool) {
        let mut x = self.flags.dbc();
        if flag {
            x |= 1;
            self.set_dbc(x);
        } else {
            x &= !1;
            self.set_dbc(x);
        }
    }
}

#[repr(C)]
pub struct HBACMDTBL {
    cfis: [u8; 64],

    acmd: [u8; 16],

    rsv: [u8; 48],

    prdt_entry: [HBAPRDTEntry; 1],
}

fn check_type(port: &HBAPort) -> u8 {
    let ssts = port.ssts;
    let ipm: u8 = ((ssts >> 8) & 0x0F) as u8;
    let det: u8 = (ssts & 0x0F) as u8;

    if det != HBA_PORT_DET_PRESENT {
        return AHCI_DEV_NULL;
    }
    
    if ipm != HBA_PORT_IPM_ACTIVE {
        return AHCI_DEV_NULL;
    }

    let sig = port.sig;

    if sig == SATA_SIG_ATAPI {
        return AHCI_DEV_SATAPI;
    } else if sig == SATA_SIG_SEMB {
        return AHCI_DEV_SEMB;
    } else if sig == SATA_SIG_PM {
        return AHCI_DEV_PM;
    } else if sig == SATA_SIG_ATA {
        return AHCI_DEV_SATA;
    } else {
        panic!("Unable to get correct SATA signature!");
    }
}

pub fn probe_port(abar: &HBAMem) {
    let mut pi = abar.pi;
    let mut i: u32 = 0;

    while i < 32 {
        if pi & 1 != 0 {
            let dt = check_type(&abar.ports[i as usize]);
            if dt == AHCI_DEV_SATA {
                println!("SATA drive found at port {}", i);
            } else if dt == AHCI_DEV_SATAPI {
                println!("SATAPI drive found at port {}", i);
            } else if dt == AHCI_DEV_SEMB {
                println!("SEMB drive found at port {}", i);
            } else if dt == AHCI_DEV_PM {
                println!("PM drive found at port {}", i);
            } else {
                println!("No drive found at port {}", i);
            }
        }

        pi >>= 1;
        i += 1;
    }
}

fn start_cmd(port: &mut HBAPort) {
    while port.cmd & HBA_PXCMD_CR != 0 {}

    port.cmd |= HBA_PXCMD_FRE;
    port.cmd |= HBA_PXCMD_ST;
}

fn stop_cmd(port: &mut HBAPort) {
    port.cmd &= !HBA_PXCMD_ST;
    port.cmd &= !HBA_PXCMD_FRE;

    loop {
        if port.cmd & HBA_PXCMD_FR != 0 {
            continue
        }

        if port.cmd & HBA_PXCMD_CR != 0 {
            continue
        }

        break
    }
}

pub fn port_rebase(port: &mut HBAPort, n: u32) {
    stop_cmd(port);

    (*port).clb = AHCI_BASE + ((n as usize) << 10) as u32;
    (*port).clbu = 0;
    unsafe { ptr::write_bytes((*port).clb as *mut u8, 0, 1024); }

    (*port).fb = AHCI_BASE + (32 << 10) + ((n as usize) << 8) as u32;
    (*port).fbu = 0;   
    unsafe { ptr::write_bytes((*port).fb as *mut u8, 0, 256); }

    let cmdheader = (*port).clb as *mut HBACMDHeader;
    for i in 0..32 {
        unsafe {
            let header = cmdheader.add(i);
            (*header).prdtl = 8;
            (*header).ctba = AHCI_BASE + (40 << 10) + ((n as usize) << 13) as u32 + (i << 8) as u32;

            ptr::write_bytes((*header).ctba as *mut u8, 0, 256);
        }
    }

    start_cmd(port);
}

fn find_cmdslot(port: &mut HBAPort) -> Option<u32> {
    let mut slots: u32 = port.sact | port.ci;
    
    for i in 0..slots {
        if (slots & 1) == 0 {
            return Some(i);
        }
        slots >>= 1;
    }
    warning!("Couldn't find free command list entry!");
    return None;
}

pub unsafe fn read(
    port: &mut HBAPort,
    startl: u32,
    starth: u32,
    count: u32,
    mut buf: *mut u16,
) -> bool {

    (*port).is = u32::MAX;

    let mut spin = 0;
    let slot = find_cmdslot(port);
    if slot.is_none() {
        return false;
    }

    let mut slot = slot.unwrap();

    let cmdheader_ptr = (*port).clb as *mut HBACMDHeader;
    let cmdheader = cmdheader_ptr.add(slot as usize);

    (*cmdheader).byte0.set_cfl((size_of::<FISRegH2D>() / 4) as u8);
    (*cmdheader).byte0.with_w(false); 
    (*cmdheader).prdtl = (((count - 1) >> 4) + 1) as u16;

    let cmdtbl = (*cmdheader).ctba as *mut HBACMDTBL;

    let tbl_size = size_of::<HBACMDTBL>() + (((*cmdheader).prdtl as usize - 1) * size_of::<HBAPRDTEntry>());
    ptr::write_bytes(cmdtbl as *mut u8, 0, tbl_size);

    let prdt_count = (*cmdheader).prdtl as usize;
    let mut remaining_count = count;

    for i in 0..(prdt_count - 1) {
        let prdt_entry = &mut (*cmdtbl).prdt_entry[i];
        prdt_entry.dba = buf as u32;
        prdt_entry.set_dbc(8 * 1024 - 1); 
        prdt_entry.set_i(true);

        buf = buf.add(4 * 1024);
        remaining_count -= 16; 
    }

    let last = prdt_count - 1;
    let last_entry = &mut (*cmdtbl).prdt_entry[last];
    last_entry.dba = buf as u32;
    last_entry.set_dbc((remaining_count << 9) - 1); 
    last_entry.set_i(true);

    let cmdfis = &mut (*cmdtbl).cfis as *mut u8 as *mut FISRegH2D;
    (*cmdfis).fis_type = 0x37;
    (*cmdfis).flags.set_c(true); 
    (*cmdfis).command = ATA_CMD_READ_DMA_EX;

    (*cmdfis).lba0 = startl as u8;
    (*cmdfis).lba1 = (startl >> 8) as u8;
    (*cmdfis).lba2 = (startl >> 16) as u8;
    (*cmdfis).device = 1 << 6; 
    (*cmdfis).lba3 = (startl >> 24) as u8;
    (*cmdfis).lba4 = starth as u8;
    (*cmdfis).lba5 = (starth >> 8) as u8;
    (*cmdfis).countl = remaining_count as u8;
    (*cmdfis).counth = ((remaining_count >> 8) & 0xFF) as u8;

    while ((*port).tfd & (ATA_DEV_BUSY as u32 | ATA_DEV_DRQ as u32) != 0) && (spin < 1_000_000) {
        spin += 1;
    }
    if spin == 1_000_000 {
        error!("Port is hung");
        return false;
    }

    (*port).ci = 1 << slot;

    loop {
        if ((*port).ci & (1 << slot)) == 0 {
            break;
        }
        if ((*port).is & HBA_PXIS_TFES) != 0 {
            error!("Read disk error");
            return false;
        }
    }

    if ((*port).is & HBA_PXIS_TFES) != 0 {
        error!("Read disk error");
        return false;
    }
    true
}

pub struct AHCIDevice {
    pub pci_device: PCIDevice,
    pub is_mounted: bool
}

impl AHCIDevice {
    pub fn new(pci_device: PCIDevice) -> Self {
        Self { pci_device, is_mounted: false }
    }
}

impl Display for AHCIDevice {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let m = bool_to_yn(self.is_mounted);
        write!(f, "PCI Device: {{ {} }} | Is Mounted: {}", self.pci_device, m)
    }
}

/// Checks for AHCI devices connected by PCI
pub fn scan_for_ahci_devs() -> Vec<AHCIDevice> {
    let devs = unsafe { scan_pci_bus() };
    let mut res: Vec<AHCIDevice> = Vec::new();

    for dev in devs {
        let d: PCIDevice = dev.into();
        if d.class_id == 0x1 && d.subclass == 0x6 && d.prog_if == 0x1 {
            res.push(AHCIDevice::new(d));
        }
    }

    return res;
}