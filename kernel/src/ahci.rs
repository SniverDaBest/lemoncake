#![allow(dead_code, private_interfaces)]

use crate::pci::{PCIDevice, read_pci, write_pci};
use crate::{error, info, serial_println};
use alloc::{vec, vec::*};
use x86_64::structures::paging::PhysFrame;
use x86_64::structures::paging::mapper::MapToError;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB},
};

const SATA_SIG_ATA: u32 = 0x00000101;
const SATA_SIG_ATAPI: u32 = 0xEB140101;
const SATA_SIG_SEMB: u32 = 0xC33C0101;
const SATA_SIG_PM: u32 = 0x96690101;

const AHCI_DEV_NULL: i32 = 0;
const AHCI_DEV_SATA: i32 = 1;
const AHCI_DEV_SEMB: i32 = 2;
const AHCI_DEV_PM: i32 = 3;
const AHCI_DEV_SATAPI: i32 = 4;

const HBA_PORT_IPM_ACTIVE: u8 = 1;
const HBA_PORT_DET_PRESENT: u8 = 3;

const AHCI_BASE: u32 = 0x400000;

const HBA_PXCMD_ST: u32 = 0x0001;
const HBA_PXCMD_FRE: u32 = 0x0010;
const HBA_PXCMD_FR: u32 = 0x4000;
const HBA_PXCMD_CR: u32 = 0x8000;
const HBA_PXIS_TFES: u32 = 1 << 30;

const ATA_DEV_BUSY: u32 = 0x80;
const ATA_DEV_DRQ: u32 = 0x08;
const ATA_CMD_READ_DMA_EX: u8 = 0x25;
const ATA_CMD_WRITE_DMA_EX: u8 = 0x35;

const SECTOR_SIZE: usize = 512;
const AHCI_BASE_ADDR: u64 = 0x400000;
const AHCI_MEMORY_SIZE: usize = 64 * 1024;
const AHCI_PORT_OFFSET: usize = 0x8000;
const AHCI_CMD_LIST_SIZE: usize = 1024;
const AHCI_FIS_SIZE: usize = 256;
const AHCI_CMD_TABLE_SIZE: usize = 256;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum FisType {
    RegH2D = 0x27,
    RegD2H = 0x34,
    DmaAct = 0x39,
    DmaSetup = 0x41,
    Data = 0x46,
    Bist = 0x58,
    PioSetup = 0x5F,
    DevBits = 0xA1,
}

#[repr(C, packed)]
struct FisRegH2D {
    fis_type: u8,
    pmport_c: u8,
    command: u8,
    featurel: u8,
    lba0: u8,
    lba1: u8,
    lba2: u8,
    device: u8,
    lba3: u8,
    lba4: u8,
    lba5: u8,
    featureh: u8,
    countl: u8,
    counth: u8,
    icc: u8,
    control: u8,
    rsv1: [u8; 4],
}

#[repr(C)]
struct HbaPort {
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

#[repr(C)]
struct HbaMem {
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
    rsv: [u8; 0xA0 - 0x2C],
    vendor: [u8; 0x100 - 0xA0],
    ports: [HbaPort; 1],
}

#[repr(C)]
struct HbaCmdHeader {
    flags: u8,
    pmp_rsv: u8,
    prdtl: u16,
    prdbc: u32,
    ctba: u32,
    ctbau: u32,
    rsv1: [u32; 4],
}

#[repr(C, packed)]
struct HbaPrdtEntry {
    dba: u32,
    dbau: u32,
    rsv0: u32,
    dbc: u32,
}

#[repr(C, packed)]
struct HbaCmdTbl {
    cfis: [u8; 64],
    acmd: [u8; 16],
    rsv: [u8; 48],
    prdt_entry: [HbaPrdtEntry; 1],
}

impl HbaPort {
    fn check_type(&self) -> i32 {
        let ssts = self.ssts;
        let ipm = ((ssts >> 8) & 0x0F) as u8;
        let det = (ssts & 0x0F) as u8;

        if det != HBA_PORT_DET_PRESENT {
            return AHCI_DEV_NULL;
        }
        if ipm != HBA_PORT_IPM_ACTIVE {
            return AHCI_DEV_NULL;
        }

        match self.sig {
            SATA_SIG_ATAPI => AHCI_DEV_SATAPI,
            SATA_SIG_SEMB => AHCI_DEV_SEMB,
            SATA_SIG_PM => AHCI_DEV_PM,
            _ => AHCI_DEV_SATA,
        }
    }

    fn start_cmd(&mut self) {
        while (self.cmd & HBA_PXCMD_CR) != 0 {}

        self.cmd |= HBA_PXCMD_FRE;
        self.cmd |= HBA_PXCMD_ST;
    }

    fn stop_cmd(&mut self) {
        self.cmd &= !HBA_PXCMD_ST;
        self.cmd &= !HBA_PXCMD_FRE;

        loop {
            if (self.cmd & HBA_PXCMD_FR) != 0 {
                continue;
            }
            if (self.cmd & HBA_PXCMD_CR) != 0 {
                continue;
            }
            break;
        }
    }

    fn read(&mut self, start: u64, sectors: u32, buf: &mut [u8]) -> bool {
        let spin_limit = 1000000;
        let mut spin = 0;
        while (self.tfd & (ATA_DEV_BUSY | ATA_DEV_DRQ)) != 0 && spin < spin_limit {
            spin += 1;
        }
        if spin == spin_limit {
            return false;
        }

        let cmd_slot = 0;
        let cmd_header =
            unsafe { &mut *((self.clb as usize + (cmd_slot << 7)) as *mut HbaCmdHeader) };
        cmd_header.flags = 1 << 7;
        cmd_header.prdtl = 1;

        let cmd_tbl = unsafe { &mut *((cmd_header.ctba as usize) as *mut HbaCmdTbl) };
        unsafe {
            core::ptr::write_bytes(
                cmd_tbl as *mut HbaCmdTbl as *mut u8,
                0,
                core::mem::size_of::<HbaCmdTbl>(),
            );
        }

        cmd_tbl.prdt_entry[0].dba = buf.as_ptr() as u32;
        cmd_tbl.prdt_entry[0].dbau = 0;
        cmd_tbl.prdt_entry[0].dbc = (sectors * SECTOR_SIZE as u32) | 1;

        let fis = unsafe { &mut *(cmd_tbl.cfis.as_ptr() as *mut FisRegH2D) };
        fis.fis_type = FisType::RegH2D as u8;
        fis.pmport_c = 1 << 7;
        fis.command = ATA_CMD_READ_DMA_EX;
        fis.lba0 = start as u8;
        fis.lba1 = (start >> 8) as u8;
        fis.lba2 = (start >> 16) as u8;
        fis.device = 1 << 6;
        fis.lba3 = (start >> 24) as u8;
        fis.lba4 = (start >> 32) as u8;
        fis.lba5 = (start >> 40) as u8;
        fis.countl = (sectors & 0xFF) as u8;
        fis.counth = ((sectors >> 8) & 0xFF) as u8;

        self.is = !0;
        self.ci = 1 << cmd_slot;

        while (self.ci & (1 << cmd_slot)) != 0 {}
        if (self.is & HBA_PXIS_TFES) != 0 {
            return false;
        }
        true
    }

    fn write(&mut self, start: u64, sectors: u32, buf: &[u8]) -> bool {
        let spin_limit = 1000000;
        let mut spin = 0;
        while (self.tfd & (ATA_DEV_BUSY | ATA_DEV_DRQ)) != 0 && spin < spin_limit {
            spin += 1;
        }
        if spin == spin_limit {
            return false;
        }

        let cmd_slot = 0;
        let cmd_header =
            unsafe { &mut *((self.clb as usize + (cmd_slot << 7)) as *mut HbaCmdHeader) };
        cmd_header.flags = (1 << 7) | (1 << 6);
        cmd_header.prdtl = 1;

        let cmd_tbl = unsafe { &mut *((cmd_header.ctba as usize) as *mut HbaCmdTbl) };
        unsafe {
            core::ptr::write_bytes(
                cmd_tbl as *mut HbaCmdTbl as *mut u8,
                0,
                core::mem::size_of::<HbaCmdTbl>(),
            );
        }

        cmd_tbl.prdt_entry[0].dba = buf.as_ptr() as u32;
        cmd_tbl.prdt_entry[0].dbau = 0;
        cmd_tbl.prdt_entry[0].dbc = (sectors * SECTOR_SIZE as u32) | 1;

        let fis = unsafe { &mut *(cmd_tbl.cfis.as_ptr() as *mut FisRegH2D) };
        fis.fis_type = FisType::RegH2D as u8;
        fis.pmport_c = 1 << 7;
        fis.command = ATA_CMD_WRITE_DMA_EX;
        fis.lba0 = start as u8;
        fis.lba1 = (start >> 8) as u8;
        fis.lba2 = (start >> 16) as u8;
        fis.device = 1 << 6;
        fis.lba3 = (start >> 24) as u8;
        fis.lba4 = (start >> 32) as u8;
        fis.lba5 = (start >> 40) as u8;
        fis.countl = (sectors & 0xFF) as u8;
        fis.counth = ((sectors >> 8) & 0xFF) as u8;

        self.is = !0;
        self.ci = 1 << cmd_slot;

        while (self.ci & (1 << cmd_slot)) != 0 {}
        if (self.is & HBA_PXIS_TFES) != 0 {
            return false;
        }
        true
    }
}

pub fn probe_port(abar: &mut HbaMem) {
    let mut pi = abar.pi;
    let mut i = 0;

    while i < 32 {
        if (pi & 1) != 0 {
            let dt = abar.ports[i as usize].check_type();
            let port_msg = match dt {
                AHCI_DEV_SATA => "SATA drive found at port ",
                AHCI_DEV_SATAPI => "SATAPI drive found at port ",
                AHCI_DEV_SEMB => "SEMB drive found at port ",
                AHCI_DEV_PM => "PM drive found at port ",
                _ => "No drive found at port ",
            };
            serial_println!("{}{}", port_msg, i);
        }
        pi >>= 1;
        i += 1;
    }
}

fn map_ahci_memory(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let pages = (AHCI_MEMORY_SIZE + 0xFFF) / 0x1000;
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE;

    for i in 0..pages {
        let page_addr = AHCI_BASE_ADDR + (i * 0x1000) as u64;
        let page = Page::containing_address(VirtAddr::new(page_addr));

        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    Ok(())
}

fn check_memory_mapping(addr: u64) -> bool {
    use x86_64::registers::control::Cr3;
    use x86_64::structures::paging::PageTable;

    let (level_4_table_frame, _) = Cr3::read();
    let level_4_table =
        unsafe { &*(level_4_table_frame.start_address().as_u64() as *const PageTable) };

    let addr = VirtAddr::new(addr);
    let l4_idx = (addr.as_u64() >> 39) & 0x1FF;
    let l3_idx = (addr.as_u64() >> 30) & 0x1FF;
    let l2_idx = (addr.as_u64() >> 21) & 0x1FF;
    let l1_idx = (addr.as_u64() >> 12) & 0x1FF;

    let l4_entry = &level_4_table[l4_idx as usize];
    if l4_entry.is_unused() {
        return false;
    }

    let l3_table = unsafe { &*(l4_entry.addr().as_u64() as *const PageTable) };
    let l3_entry = &l3_table[l3_idx as usize];
    if l3_entry.is_unused() {
        return false;
    }

    let l2_table = unsafe { &*(l3_entry.addr().as_u64() as *const PageTable) };
    let l2_entry = &l2_table[l2_idx as usize];
    if l2_entry.is_unused() {
        return false;
    }

    let l1_table = unsafe { &*(l2_entry.addr().as_u64() as *const PageTable) };
    let l1_entry = &l1_table[l1_idx as usize];

    return !l1_entry.is_unused();
}

#[derive(Debug)]
pub struct AhciPort {
    pub port_number: usize,
    pub port_type: i32,
    pub hba_port: *mut HbaPort,
    pub is_implemented: bool,
}

#[derive(Debug)]
pub struct AhciDevice {
    pub pci_device: PCIDevice,
    pub abar: *mut HbaMem,
    pub ports: Vec<AhciPort>,
}

fn init_port_with_timeout(port: &mut AhciPort) -> bool {
    const TIMEOUT: u64 = 1_000_000;
    let mut timeout = TIMEOUT;

    unsafe {
        (*port.hba_port).stop_cmd();

        while timeout > 0 && ((*port.hba_port).cmd & HBA_PXCMD_CR) != 0 {
            timeout -= 1;
            if timeout == 0 {
                info!("Port stop command timed out");
                return false;
            }
        }

        port_rebase(port.hba_port, port.port_number);

        (*port.hba_port).start_cmd();

        timeout = TIMEOUT;
        while timeout > 0 && ((*port.hba_port).cmd & HBA_PXCMD_CR) == 0 {
            timeout -= 1;
            if timeout == 0 {
                info!("Port start command timed out");
                return false;
            }
        }
    }
    true
}

impl AhciDevice {
    pub fn new(
        pci_device: PCIDevice,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Option<Self> {
        if pci_device.class_code != 0x01 || pci_device.subclass != 0x06 {
            return None;
        }

        let abar_phys = read_pci(0x24, &pci_device) & !0xF;

        let pages = 256;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE;

        let abar_virt = VirtAddr::new(0x_4000_0000);

        for i in 0..pages {
            let page = Page::containing_address(abar_virt + (i * 0x1000));
            let frame =
                PhysFrame::containing_address(PhysAddr::new(abar_phys as u64 + (i * 0x1000)));
            unsafe {
                mapper
                    .map_to(page, frame, flags, frame_allocator)
                    .expect("Failed to map AHCI ABAR")
                    .flush();
            }
        }

        let abar = abar_virt.as_mut_ptr::<HbaMem>();

        let mut device = AhciDevice {
            pci_device,
            abar,
            ports: Vec::new(),
        };

        device.init_ports();
        Some(device)
    }

    fn init_ports(&mut self) {
        let hba = unsafe { &mut *self.abar };
        let pi = hba.pi;

        for i in 0..32 {
            if (pi & (1 << i)) != 0 {
                if hba.ports.len() <= i {
                    break;
                }
                let port = &mut hba.ports[i];
                let port_type = port.check_type();

                self.ports.push(AhciPort {
                    port_number: i,
                    port_type,
                    hba_port: port as *mut HbaPort,
                    is_implemented: true,
                });
            }
        }
    }

    pub fn init(
        &mut self,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<(), MapToError<Size4KiB>> {
        info!("Starting AHCI device initialization...");

        map_ahci_memory(mapper, frame_allocator)?;
        info!("AHCI memory mapping completed");

        let command = read_pci(0x04, &self.pci_device);
        info!("Current PCI command: 0x{:x}", command);
        write_pci(0x04, &self.pci_device, command | 0x6);
        info!("PCI bus mastering enabled");

        let ghc = unsafe { (*self.abar).ghc };
        info!("Current GHC: 0x{:x}", ghc);
        unsafe {
            (*self.abar).ghc = ghc | (1 << 31);
            info!("AHCI enabled in GHC: 0x{:x}", (*self.abar).ghc);
        }

        for port in &mut self.ports {
            if port.is_implemented && port.port_type == AHCI_DEV_SATA {
                info!("Initializing port {}", port.port_number);
                let result = init_port_with_timeout(port);
                if !result {
                    info!("Port {} initialization timed out", port.port_number);
                    continue;
                }
            }
        }

        info!("AHCI device initialization completed");
        Ok(())
    }

    /// False if bad, true if good
    pub fn read_sector(&mut self, port_number: usize, lba: u64, buffer: &mut [u8]) -> bool {
        if buffer.len() < SECTOR_SIZE {
            error!("Buffer too small for sector read");
            return false;
        }

        let port = match self.ports.iter_mut().find(|p| p.port_number == port_number) {
            Some(p) if p.port_type == AHCI_DEV_SATA => unsafe { &mut *p.hba_port },
            _ => {
                error!("Invalid port number or port is not SATA");
                return false;
            }
        };

        if !port.read(lba, 1, &mut buffer[..SECTOR_SIZE]) {
            error!("Failed to read sector at LBA {}", lba);
            return false;
        }

        return true;
    }

    /// False if bad, true if good
    pub fn write_sector(&mut self, port_number: usize, lba: u64, buffer: &[u8]) -> bool {
        if buffer.len() != SECTOR_SIZE {
            error!("Buffer not the correct size for sector write");
            return false;
        }

        let port = match self.ports.iter_mut().find(|p| p.port_number == port_number) {
            Some(p) if p.port_type == AHCI_DEV_SATA => unsafe { &mut *p.hba_port },
            _ => {
                error!("Invalid port number or port is not SATA");
                return false;
            }
        };

        if !port.write(lba, 1, &buffer[..SECTOR_SIZE]) {
            error!("Failed to write sector at LBA {}", lba);
            return false;
        }

        return true;
    }
}

pub unsafe fn find_ahci_devices(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Vec<AhciDevice> {
    let mut ahci_devices = Vec::new();

    for pci_device in crate::pci::scan_pci_bus() {
        if pci_device.class_code == 0x01 && pci_device.subclass == 0x06 {
            if let Some(ahci_device) = AhciDevice::new(pci_device, mapper, frame_allocator) {
                ahci_devices.push(ahci_device);
            }
        }
    }

    ahci_devices
}

fn port_rebase(port: *mut HbaPort, portno: usize) {
    unsafe {
        info!("Rebasing port {} at address {:p}", portno, port);

        let cmd_list_base = AHCI_BASE + (portno as u32 * AHCI_CMD_LIST_SIZE as u32);
        let fis_base =
            AHCI_BASE + (32 * AHCI_CMD_LIST_SIZE) as u32 + (portno as u32 * AHCI_FIS_SIZE as u32);

        info!("Command list base: 0x{:x}", cmd_list_base);
        info!("FIS base: 0x{:x}", fis_base);

        (*port).clb = cmd_list_base;
        (*port).clbu = 0;

        (*port).fb = fis_base;
        (*port).fbu = 0;

        let cmdheader = (*port).clb as *mut HbaCmdHeader;
        core::ptr::write_bytes(cmdheader as *mut u8, 0, AHCI_CMD_LIST_SIZE);

        for i in 0..32 {
            (*cmdheader.add(i)).prdtl = 8;
            let cmd_table_base = AHCI_BASE
                + (40 * 1024) as u32
                + (portno as u32 * 32 * AHCI_CMD_TABLE_SIZE as u32)
                + (i as u32 * AHCI_CMD_TABLE_SIZE as u32);
            (*cmdheader.add(i)).ctba = cmd_table_base;
            (*cmdheader.add(i)).ctbau = 0;
            core::ptr::write_bytes((*cmdheader.add(i)).ctba as *mut u8, 0, AHCI_CMD_TABLE_SIZE);
        }
    }
}

pub fn test_ahci_read_write(device: &mut AhciDevice) {
    let mut read_buffer = vec![0u8; SECTOR_SIZE];
    let write_buffer = vec![0xAAu8; SECTOR_SIZE];

    for port in &mut device.ports {
        if port.port_type == AHCI_DEV_SATA {
            info!("Testing SATA port {}", port.port_number);

            let port = unsafe { &mut *port.hba_port };
            if port.read(0, 1, &mut read_buffer) {
                info!("Successfully read first sector");

                if port.write(1, 1, &write_buffer) {
                    info!("Successfully wrote test pattern");

                    if port.read(1, 1, &mut read_buffer) && read_buffer == write_buffer {
                        info!("Successfully verified written data");
                    } else {
                        error!("Data verification failed.");
                    }
                } else {
                    error!("Failed to write test pattern to second sector.");
                }
            } else {
                error!("Unable to read first sector.");
            }
            break;
        }
    }
}
