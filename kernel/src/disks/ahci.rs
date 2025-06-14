use super::*;
use crate::{
    error, info, nftodo,
    pci::{PCIDevice, bar5, scan_pci_bus},
};
use alloc::{boxed::Box, vec::Vec};
use bitfield::bitfield;
use core::slice::from_raw_parts;
use volatile::Volatile;
use x86_64::structures::paging::{FrameAllocator, Mapper, PageTableFlags};
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{Page, Size4KiB},
};

#[repr(u8)]
enum FisType {
    FisTypeRegH2D = 0x27,
    FisTypeRegD2H = 0x34,
    FisTypeDmaAct = 0x39,
    FisTypeDmaSetup = 0x41,
    FisTypeData = 0x46,
    FisTypeBist = 0x58,
    FisTypePioSetup = 0x5F,
    FisTypeDevBits = 0xA1,
}

impl FisType {
    fn as_u8(self) -> u8 {
        return self as u8;
    }
}

#[repr(C)]
struct FisRegH2D {
    fis_type: u8,
    pmc: PmC,
    cmd: u8,
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

bitfield! {
    #[derive(Copy, Clone)]
    struct PmC(u8);
    impl Debug;
    pmport, set_pmport: 3, 0;
    rsv0, set_rsv0: 6, 4;
    c, set_c: 7;
}

#[repr(C)]
struct FisRegD2H {
    fis_type: u8,
    pmpi: PmpI,
    status: u8,
    err: u8,
    lba0: u8,
    lba1: u8,
    lba2: u8,
    device: u8,
    lba3: u8,
    lba4: u8,
    lba5: u8,
    rsv2: u8,
    countl: u8,
    counth: u8,
    rsv3: [u8; 2],
    rsv4: [u8; 4],
}

bitfield! {
    #[derive(Copy, Clone)]
    struct PmpI(u8);
    impl Debug;
    pmport, set_pmport: 3, 0;
    rsv0, set_rsv0: 4,5;
    i, set_i: 6;
    rsv1, set_rsv1: 7;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FisDataHdr {
    fis_type: u8,
    pmp: Pmp,
    rsv1: [u8; 2],
}

bitfield! {
    #[derive(Copy, Clone)]
    struct Pmp(u8);
    impl Debug;
    pmport, set_pmport: 0,3;
    rsv0, set_rsv0: 4, 7;
}

/// This is not C-safe code!\
/// Please combine the header (`FisDataHdr`) with the data from this struct.
struct FisData {
    fdh: FisDataHdr,
    data: &'static [u32],
}

impl FisData {
    unsafe fn from_raw(data: &[u8]) -> Option<Self> {
        if data.len() < size_of::<FisDataHdr>() {
            error!(
                "(AHCI) The buffer size is too small! ({} < {})",
                data.len(),
                size_of::<FisDataHdr>()
            );
            return None;
        }

        let hdr = unsafe { &*(data.as_ptr() as *const FisDataHdr) };
        let d = &data[size_of::<FisDataHdr>()..];
        let align = align_of::<u32>();
        if d.as_ptr() as usize & align != 0 {
            error!("(AHCI) Data is incorrectly sized!");
            return None;
        }

        let c = d.len() / 4;
        let rd = from_raw_parts(d.as_ptr() as *const u32, c);

        return Some(Self {
            fdh: *hdr,
            data: rd,
        });
    }
}

#[repr(C)]
struct FisPioSetup {
    fis_type: u8,
    pmdi: PmDI,
    status: u8,
    error: u8,
    lba0: u8,
    lba1: u8,
    lba2: u8,
    device: u8,
    lba3: u8,
    lba4: u8,
    lba5: u8,
    rsv2: u8,
    countl: u8,
    counth: u8,
    rsv3: u8,
    e_stat: u8,
    tc: u16,
    rsv4: [u8; 2],
}

bitfield! {
    #[derive(Copy, Clone)]
    struct PmDI(u8);
    pmport, set_pmport: 0, 3;
    rsv0, set_rsv0: 4;
    d, set_d: 5;
    i, set_i: 6;
    rsv1, set_rsv1: 7;
}

struct FisDmaSetup {
    fis_type: u8,
    pmdia: PmDIA,
    rsv1: [u8; 2],
    dma_buf_id: u64,
    rsv2: u32,
    dma_buf_offset: u32,
    transfer_count: u32,
    rsv3: u32,
}

bitfield! {
    #[derive(Clone, Copy)]
    struct PmDIA(u8);
    pmport, set_pmport: 0, 3;
    rsv0, set_rsv0: 4;
    d, set_d: 5;
    i, set_i: 6;
    a, set_a: 7;
}

/// This is a **volatile** struct!
#[repr(C)]
#[derive(Debug)]
struct HbaPort {
    clb: Volatile<u32>,
    clbu: Volatile<u32>,
    fb: Volatile<u32>,
    fbu: Volatile<u32>,
    is: Volatile<u32>,
    ie: Volatile<u32>,
    cmd: Volatile<u32>,
    rsv0: Volatile<u32>,
    tfd: Volatile<u32>,
    sig: Volatile<u32>,
    ssts: Volatile<u32>,
    sctl: Volatile<u32>,
    serr: Volatile<u32>,
    sact: Volatile<u32>,
    ci: Volatile<u32>,
    sntf: Volatile<u32>,
    fbs: Volatile<u32>,
    rsv1: Volatile<[u32; 11]>,
    vendor: Volatile<[u32; 4]>,
}

/// This is a **volatile** struct!
#[repr(C)]
#[derive(Debug)]
struct HbaMemHdr {
    cap: Volatile<u32>,
    ghc: Volatile<u32>,
    is: Volatile<u32>,
    pi: Volatile<u32>,
    vs: Volatile<u32>,
    ccc_ctl: Volatile<u32>,
    ccc_pts: Volatile<u32>,
    em_loc: Volatile<u32>,
    em_ctl: Volatile<u32>,
    cap2: Volatile<u32>,
    bohc: Volatile<u32>,
    rsv0: Volatile<[u8; 0xA0 - 0x2C]>,
    vendor: Volatile<[u8; 0x100 - 0xA0]>,
}

/// This is not C-safe code!\
/// Please combine the header (`FisDataHdr`) with the ports from this struct.
#[derive(Debug)]
struct HbaMem {
    hdr: &'static mut HbaMemHdr,
    ports: &'static [HbaPort],
}

impl HbaMem {
    unsafe fn from_raw(data: &[u8]) -> Option<Self> {
        if data.len() < size_of::<HbaMemHdr>() {
            error!(
                "(AHCI) The buffer size is too small! ({} < {})",
                data.len(),
                size_of::<HbaMemHdr>()
            );
            return None;
        }

        let hdr = unsafe { &mut *(data.as_ptr() as *mut HbaMemHdr) };
        let d = &data[size_of::<HbaMemHdr>()..];
        let align = align_of::<u32>();
        if d.as_ptr() as usize & align != 0 {
            error!("(AHCI) Data is incorrectly sized!");
            return None;
        }

        let c = d.len() / 4;
        let rd = from_raw_parts(d.as_ptr() as *const HbaPort, c);

        return Some(Self {
            hdr: hdr,
            ports: rd,
        });
    }
}

#[repr(C)]
struct FisDevBits {
    fis_type: u8,
    pmpin: PmpIN,
    stlsth: StlSth,
    error: u8,
}

bitfield! {
    #[derive(Copy, Clone)]
    struct PmpIN(u8);
    pmport, set_pmport: 0, 3;
    rsv0, set_rsv0: 4, 5;
    i, set_i: 6;
    n, set_n: 7;
}

bitfield! {
    #[derive(Copy, Clone)]
    struct StlSth(u8);
    statusl, set_statusl: 0, 2;
    rsv1, set_rsv1: 3;
    statush, set_statush: 4, 6;
    rsv2, set_rsv2: 7;
}

#[repr(C)]
struct HbaFis {
    dsfis: FisDmaSetup,
    pad0: [u8; 4],
    psfis: FisPioSetup,
    pad1: [u8; 12],
    rfis: FisRegD2H,
    pad2: [u8; 4],
    sdbfis: FisDevBits,
    ufis: [u8; 64],
    rsv: [u8; 0x100 - 0xA0],
}

#[repr(C)]
struct HbaCmdHeader {
    cflawp: CflAWP,
    rbcpmp: RBCPmp,
    prdtl: u16,
    prdbc: Volatile<u32>,
    ctba: u32,
    ctbau: u32,
    rsv1: [u32; 4],
}

bitfield! {
    #[derive(Clone, Copy)]
    struct CflAWP(u8);
    cfl, set_cfl: 0, 4;
    a, set_a: 5;
    w, set_w: 6;
    p, set_p: 7;
}

bitfield! {
    #[derive(Clone, Copy)]
    struct RBCPmp(u8);
    r, set_r: 0;
    b, set_b: 1;
    c, set_c: 2;
    rsv0, set_rsv0: 3;
    pmp, set_pmp: 4,7;
}

#[repr(C)]
struct HbaPrdtEntry {
    dba: u32,
    dbau: u32,
    rsv0: u32,
    dbci: DbcI,
}

bitfield! {
    #[derive(Clone, Copy)]
    struct DbcI(u32);
    dbc, set_dbc: 0, 21;
    rsv1, set_rsv1: 22, 30;
    i, set_i: 31;
}

#[repr(C)]
struct HbaCmdTbl {
    cfis: [u8; 64],
    acmd: [u8; 16],
    rsv0: [u8; 48],
    prdt_ent: HbaPrdtEntry,
}

pub fn is_ahci(device: &PCIDevice) -> bool {
    return if device.class_code == 0x01 && device.subclass == 0x06 {
        true
    } else {
        false
    };
}

pub fn scan_for_ahci_controllers() -> Vec<PCIDevice> {
    let mut devices: Vec<PCIDevice> = Vec::new();
    for device in scan_pci_bus() {
        if is_ahci(&device) {
            info!("(AHCI) Found AHCI device on PCI!");
            devices.push(device);
        }
    }
    return devices;
}

fn check_type(port: &HbaPort) -> u32 {
    let ssts = port.ssts.read();
    let ipm = ((ssts >> 8) & 0x0F) as u8;
    let det = (ssts & 0x0F) as u8;

    if det != HBA_PORT_DET_PRESENT || ipm != HBA_PORT_IPM_ACTIVE {
        return AHCI_DEV_NULL;
    }

    match port.sig.read() {
        SATA_SIG_ATA => return AHCI_DEV_SATA,
        SATA_SIG_ATAPI => return AHCI_DEV_SATAPI,
        SATA_SIG_SEMB => return AHCI_DEV_SEMB,
        SATA_SIG_PM => return AHCI_DEV_PM,
        _ => return AHCI_DEV_SATA,
    }
}

fn probe_port(abar: &HbaMem) -> Vec<Option<(u32, usize)>> {
    let mut ret: Vec<Option<(u32, usize)>> = Vec::new();

    let mut pi = abar.hdr.pi.read();
    for i in 0..31 {
        if pi & 1 != 0 {
            if abar.ports.len() - 1 < i {
                break;
            }

            let dt = check_type(&abar.ports[i]);
            match dt {
                AHCI_DEV_SATA => {
                    ret.push(Some((AHCI_DEV_SATA, i)));
                }
                AHCI_DEV_SATAPI => {
                    ret.push(Some((AHCI_DEV_SATAPI, i)));
                }
                AHCI_DEV_SEMB => {
                    ret.push(Some((AHCI_DEV_SEMB, i)));
                }
                AHCI_DEV_PM => {
                    ret.push(Some((AHCI_DEV_PM, i)));
                }
                _ => {
                    ret.push(None);
                }
            }

            pi >>= 1;
        }
    }

    ret
}

fn start_cmd(port: &mut HbaPort) {
    while port.cmd.read() & HBA_PXCMD_CR != 0 {}

    port.cmd.write(port.cmd.read() | HBA_PXCMD_FRE);
    port.cmd.write(port.cmd.read() | HBA_PXCMD_ST);
}

fn stop_cmd(port: &mut HbaPort) {
    port.cmd.write(port.cmd.read() & !HBA_PXCMD_ST);
    port.cmd.write(port.cmd.read() & !HBA_PXCMD_FRE);

    loop {
        if port.cmd.read() & HBA_PXCMD_FR != 0 || port.cmd.read() & HBA_PXCMD_CR != 0 {
            continue;
        }
        break;
    }
}

unsafe fn port_rebase(port: &mut HbaPort, pn: u32) {
    stop_cmd(port);

    port.clb.write(AHCI_BASE + (pn << 10));
    port.clbu.write(0);
    core::ptr::write_bytes(port.clb.read() as *mut u8, 0, 1024);

    port.fb.write(AHCI_BASE + (32 << 10) + (pn << 8));
    port.fbu.write(0);
    core::ptr::write_bytes(port.fb.read() as *mut u8, 0, 1024);

    let cmdhdr = port.clb.read() as *mut HbaCmdHeader;
    for i in 0..32 {
        (*cmdhdr.add(i)).prdtl = 8;
        (*cmdhdr.add(i)).ctba = AHCI_BASE + (40 << 10) + (pn << 13) + ((i as u32) << 8);
        (*cmdhdr.add(i)).ctbau = 8;
        core::ptr::write_bytes((*cmdhdr.add(i)).ctba as *mut u8, 0, 256);
    }

    start_cmd(port);
}

/// Return true on succeed, return false on fail.
unsafe fn read(
    port: &mut HbaPort,
    startl: u32,
    starth: u32,
    mut count: u32,
    mut buf: &mut [u16],
) -> bool {
    port.is.write(u32::MAX);
    let mut spin = 0;
    let slot = find_cmdslot(port);
    if slot == -1 {
        return false;
    }

    let cmdhdr = &mut *((port.clb.read() as *mut HbaCmdHeader).add(slot as usize));
    cmdhdr.cflawp.set_cfl(size_of::<FisRegH2D>() as u8);
    cmdhdr.cflawp.set_w(false);
    cmdhdr.prdtl = (((count - 1) >> 4) + 1) as u16;

    let cmdtbl = cmdhdr.ctba as *mut HbaCmdTbl;
    core::ptr::write_bytes(
        cmdtbl as *mut u8,
        0,
        size_of::<HbaCmdTbl>() + (cmdhdr.prdtl as usize - 1) * size_of::<HbaPrdtEntry>(),
    );

    for i in 0..cmdhdr.prdtl - 1 {
        let prdt_entry = (cmdtbl as *mut HbaPrdtEntry).add(i as usize);
        (*prdt_entry).dba = buf.as_ptr() as u32;
        (*prdt_entry).dbci.set_dbc(8191);
        (*prdt_entry).dbci.set_i(false);
        buf = &mut buf[2048..];
        count -= 16;
    }

    let prdt_entry = (cmdtbl as *mut HbaPrdtEntry).add(cmdhdr.prdtl as usize - 1);
    (*prdt_entry).dba = buf.as_ptr() as u32;
    (*prdt_entry).dbci.set_dbc((count << 9) - 1);
    (*prdt_entry).dbci.set_i(true);

    let cmdfis = &mut *((*cmdtbl).cfis.as_mut_ptr() as *mut FisRegH2D);
    cmdfis.fis_type = FisType::FisTypeRegH2D.as_u8();
    cmdfis.pmc.set_c(true);
    cmdfis.cmd = ATA_CMD_READ_DMA_EX;

    cmdfis.lba0 = startl as u8;
    cmdfis.lba1 = (startl >> 8) as u8;
    cmdfis.lba2 = (startl >> 16) as u8;
    cmdfis.device = 1 << 6;

    cmdfis.lba3 = (startl >> 24) as u8;
    cmdfis.lba4 = starth as u8;
    cmdfis.lba5 = (starth >> 8) as u8;

    cmdfis.countl = (count & 0xFF) as u8;
    cmdfis.countl = (count >> 8 & 0xFF) as u8;

    while port.tfd.read() & (ATA_DEV_BUSY | ATA_DEV_DRQ) as u32 != 0 && spin < 1_000_000 {
        spin += 1;
    }

    if spin == 1_000_000 {
        error!("(AHCI) Port is hung!");
        return false;
    }

    port.ci.write(1 << slot);

    loop {
        if port.ci.read() & 1 << slot == 0 {
            break;
        }
        if port.is.read() & HBA_PXIS_TFES as u32 != 0 {
            error!("(AHCI) Unable to read disk!");
            return false;
        }
    }

    if port.is.read() & HBA_PXIS_TFES as u32 != 0 {
        error!("(AHCI) Unable to read disk!");
        return false;
    }

    return true;
}

unsafe fn write(
    port: &mut HbaPort,
    startl: u32,
    starth: u32,
    mut count: u32,
    mut buf: &[u16],
) -> bool {
    port.is.write(u32::MAX);
    let mut spin = 0;
    let slot = find_cmdslot(port);
    if slot == -1 {
        return false;
    }

    let cmdhdr = &mut *((port.clb.read() as *mut HbaCmdHeader).add(slot as usize));
    cmdhdr
        .cflawp
        .set_cfl(core::mem::size_of::<FisRegH2D>() as u8);
    cmdhdr.cflawp.set_w(true);
    cmdhdr.prdtl = (((count - 1) >> 4) + 1) as u16;

    let cmdtbl = cmdhdr.ctba as *mut HbaCmdTbl;
    core::ptr::write_bytes(
        cmdtbl as *mut u8,
        0,
        core::mem::size_of::<HbaCmdTbl>()
            + (cmdhdr.prdtl as usize - 1) * core::mem::size_of::<HbaPrdtEntry>(),
    );

    for i in 0..cmdhdr.prdtl - 1 {
        let prdt_entry = (cmdtbl as *mut HbaPrdtEntry).add(i as usize);
        (*prdt_entry).dba = buf.as_ptr() as u32;
        (*prdt_entry).dbci.set_dbc(8191);
        (*prdt_entry).dbci.set_i(false);
        buf = &buf[2048..];
        count -= 16;
    }

    let prdt_entry = (cmdtbl as *mut HbaPrdtEntry).add(cmdhdr.prdtl as usize - 1);
    (*prdt_entry).dba = buf.as_ptr() as u32;
    (*prdt_entry).dbci.set_dbc((count << 9) - 1);
    (*prdt_entry).dbci.set_i(true);

    let cmdfis = &mut *((*cmdtbl).cfis.as_mut_ptr() as *mut FisRegH2D);
    cmdfis.fis_type = FisType::FisTypeRegH2D.as_u8();
    cmdfis.pmc.set_c(true);
    cmdfis.cmd = ATA_CMD_WRITE_DMA_EX;

    cmdfis.lba0 = startl as u8;
    cmdfis.lba1 = (startl >> 8) as u8;
    cmdfis.lba2 = (startl >> 16) as u8;
    cmdfis.device = 1 << 6;

    cmdfis.lba3 = (startl >> 24) as u8;
    cmdfis.lba4 = starth as u8;
    cmdfis.lba5 = (starth >> 8) as u8;

    cmdfis.countl = (count & 0xFF) as u8;
    cmdfis.counth = ((count >> 8) & 0xFF) as u8;

    while port.tfd.read() & (ATA_DEV_BUSY | ATA_DEV_DRQ) as u32 != 0 && spin < 1_000_000 {
        spin += 1;
    }
    if spin == 1_000_000 {
        error!("(AHCI) Port is hung!");
        return false;
    }

    port.ci.write(1 << slot);

    loop {
        if port.ci.read() & (1 << slot) == 0 {
            break;
        }
        if port.is.read() & HBA_PXIS_TFES as u32 != 0 {
            error!("(AHCI) Unable to write disk!");
            return false;
        }
    }

    if port.is.read() & HBA_PXIS_TFES as u32 != 0 {
        error!("(AHCI) Unable to write disk!");
        return false;
    }

    true
}

fn find_cmdslot(port: &HbaPort) -> i32 {
    let mut slots = port.sact.read() | port.ci.read();
    for i in 0..slots {
        if slots & 1 == 0 {
            return i as i32;
        }
        slots >>= 1;
    }
    error!("(AHCI) Can't find free cmd list entry!");
    return -1;
}

#[allow(private_interfaces)]
#[derive(Debug)]
pub struct AHCIPort {
    pub port_num: usize,
    pub dev_type: u32,
    pub port: &'static mut HbaPort,
}

#[allow(private_interfaces)]
pub struct AHCIController {
    pub pci_dev: PCIDevice,
    pub abar: &'static mut HbaMem,
    pub ports: Vec<AHCIPort>,
}

impl AHCIController {
    pub unsafe fn from_pci(
        pci_dev: PCIDevice,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Option<Self> {
        if !is_ahci(&pci_dev) {
            error!("(AHCI) PCI device is NOT an AHCI device!");
            return None;
        }

        let abar_addr = bar5(&pci_dev);
        info!("(AHCI) BAR5 (ABAR) address: 0x{:x}", abar_addr);

        info!("(AHCI) Mapping the ABAR...");
        let abar_phys = PhysAddr::new(abar_addr as u64);
        let abar_size = core::mem::size_of::<HbaMemHdr>() + 32 * core::mem::size_of::<HbaPort>();
        let abar_start_page = Page::containing_address(VirtAddr::new(abar_phys.as_u64()));
        let abar_end_page =
            Page::containing_address(VirtAddr::new(abar_phys.as_u64() + abar_size as u64 - 1));
        let page_range = Page::range_inclusive(abar_start_page, abar_end_page);

        for page in page_range {
            match mapper.translate_page(page) {
                Ok(_) => {
                    continue;
                }
                Err(_) => {
                    let frame = frame_allocator
                        .allocate_frame()
                        .expect("(AHCI) Unable to allocate a frame!");
                    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
                    mapper
                        .map_to(page, frame, flags, frame_allocator)
                        .expect("(AHCI) Unable to map the ABAR!")
                        .flush();
                }
            }
        }

        let abar_slice = core::slice::from_raw_parts_mut(
            abar_addr as *mut u8,
            core::mem::size_of::<HbaMemHdr>() + 32 * core::mem::size_of::<HbaPort>(),
        );
        let abar_box = Box::new(
            HbaMem::from_raw(abar_slice).expect("(AHCI) Unable to put the ABAR on the heap!"),
        );
        let abar_static: &'static mut HbaMem = Box::leak(abar_box);
        info!("ABAR Address: 0x{:x}", abar_addr);

        let ghc_val = abar_static.hdr.ghc.read();
        info!(
            "(AHCI) GHC reg: 0x{:08x} (AHCI Enable bit: {})",
            ghc_val,
            if (ghc_val & (1 << 31)) != 0 {
                "set"
            } else {
                "NOT set"
            }
        );
        if (ghc_val & (1 << 31)) == 0 {
            error!("(AHCI) GHC.AE (bit 31) is NOT set! Controller is not in AHCI mode.");
        }

        probe_port(abar_static);

        let pi_val = abar_static.hdr.pi.read();
        info!("(AHCI) PI reg: 0x{:08x}", pi_val);

        let mut ports = Vec::new();
        let mut pi = pi_val;
        for i in 0..32 {
            if pi & 1 != 0 {
                if abar_static.ports.len() <= i {
                    break;
                }
                let dev_type = check_type(&abar_static.ports[i]);
                if dev_type != AHCI_DEV_NULL {
                    ports.push(AHCIPort {
                        port_num: i,
                        dev_type,
                        port: &mut *(abar_static.ports.as_ptr().add(i) as *mut HbaPort),
                    });
                }
            }
            pi >>= 1;
        }
        info!("(AHCI) from_pci: {} usable ports found", ports.len());
        return Some(Self {
            pci_dev,
            abar: abar_static,
            ports,
        });
    }

    unsafe fn initialize_ports(&mut self) {
        for p in &mut self.ports {
            port_rebase(p.port, p.port_num as u32);
        }
    }

    /// True on success, false on fail.
    pub unsafe fn read_sectors(
        &mut self,
        port_num: usize,
        lba: u32,
        count: u32,
        buf: &mut [u16],
    ) -> bool {
        match self.ports.iter_mut().find(|p| p.port_num == port_num) {
            Some(p) => read(p.port, lba, 0, count, buf),
            None => {
                error!("(AHCI) Port {} not found!", port_num);
                return false;
            }
        }
    }

    pub unsafe fn init(&mut self) {
        self.abar
            .hdr
            .ghc
            .write(self.abar.hdr.ghc.read() | (1 << 31));
        self.initialize_ports();

        for port in &mut self.ports {
            port.port.is.write(u32::MAX);

            info!(
                "(AHCI) Port {}: Device type {}",
                port.port_num, port.dev_type
            );
        }
    }

    /// True on success, false on fail.
    pub fn read_sector(&mut self, port_num: usize, lba: u64, buf: &mut [u8]) -> bool {
        let port = match self.ports.iter_mut().find(|p| p.port_num == port_num) {
            Some(p) => p,
            None => {
                error!("(AHCI) Port {} not found!", port_num);
                return false;
            }
        };

        if buf.len() != 512 {
            error!("(AHCI) Buffer length is > 512!");
            return false;
        }

        info!("(AHCI) read_sector: port_num={}, lba={}", port_num, lba);
        let result = unsafe {
            read(
                port.port,
                lba as u32,
                0,
                1,
                core::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u16, 256),
            )
        };
        if !result {
            error!(
                "(AHCI) read_sector failed: port_num={}, lba={}",
                port_num, lba
            );
        }
        result
    }

    /// True on success, false on fail.
    pub fn write_sector(&mut self, port_num: usize, lba: u64, buf: &[u8]) -> bool {
        let port = match self.ports.iter_mut().find(|p| p.port_num == port_num) {
            Some(p) => p,
            None => return false,
        };

        if buf.len() != 512 {
            error!("(AHCI) Buffer length is > 512!");
            return false;
        }

        return unsafe {
            write(
                port.port,
                lba as u32,
                0,
                1,
                core::slice::from_raw_parts(buf.as_ptr() as *const u16, 256),
            )
        };
    }
}
