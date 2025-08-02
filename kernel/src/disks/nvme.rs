use super::*;
use crate::{
    allocator::{alloc, alloc_pages, free, map_page, unmap_page},
    error, info, nftodo,
    pci::{PCIDevice, scan_pci_bus},
    sleep::Sleep,
};
use core::{
    mem::zeroed,
    ptr::{self, copy_nonoverlapping, read_volatile, write_bytes, write_volatile},
    sync::atomic::{Ordering, fence},
};
use strum_macros::FromRepr;
use volatile::Volatile;
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, Page, PageSize, PageTableFlags, Size4KiB},
};

#[repr(C, packed)]
struct NVMeCommand {
    opcode: u8,
    flags: u8,
    command_id: u16,
    nsid: u32,
    rsv0: u64,
    meta: u64,
    prp1: u64,
    prp2: u64,
    cdw10: u32,
    cdw11: u32,
    cdw12: u32,
    cdw13: u32,
    cdw14: u32,
    cdw15: u32,
}

#[repr(C, packed)]
struct NVMeCompletion {
    result: u32,
    rsv0: u32,
    sq_head: u16,
    sq_id: u16,
    cmd_id: u16,
    stat: u16,
}

struct NVMeQueue {
    sq_cmds: *const NVMeCommand,
    cqes: *mut NVMeCompletion,
    sq_db: &'static mut u32,
    cq_db: &'static mut u32,
    sq_tail: u16,
    cq_head: u16,
    qid: u16,
    q_depth: u16,
    cq_phase: u8,
    sqe_shift: u8,
    cqe_shift: u8,
}

#[repr(C, packed)]
struct NVMeIDCtrl {
    vid: u16,
    ssvid: u16,
    sn: [i8; 20],
    mn: [i8; 40],
    fr: [i8; 8],
    rab: u8,
    ieee: [u8; 3],
    cmic: u8,
    mdts: u8,
    cntlid: u16,
    ver: u32,
    rtd3r: u32,
    rtd3e: u32,
    oaes: u32,
    ctratt: u32,
    rsv0: [u8; 156],
    oacs: u16,
    acl: u8,
    aerl: u8,
    frmw: u8,
    lpa: u8,
    elpe: u8,
    npss: u8,
    avscc: u8,
    apsta: u8,
    wctemp: u16,
    cctemp: u16,
    mtfa: u16,
    hmpre: u32,
    hmmin: u32,
    tnvmcap: [u8; 16],
    unvmcap: [u8; 16],
    rpmbs: u32,
    edstt: u16,
    dsto: u8,
    fwug: u8,
    kas: u16,
    hctma: u16,
    mntmt: u16,
    mxtmt: u16,
    sanicap: u32,
    rsv1: [u8; 180],
    sqes: u8,
    cqes: u8,
    maxcmp: u16,
    nn: u32,
    oncs: u16,
    fuses: u16,
    fna: u8,
    vwc: u8,
    awun: u16,
    awupf: u16,
    nvscc: u8,
    rsv2: u8,
    acwu: u16,
    rsv3: u16,
    sgls: u32,
    rsv4: [u8; 228],
    subnqn: [i8; 256],
    rsv5: [u8; 768],
    nvmsr: [u8; 256],
    vs: [u8; 1024],
}

#[repr(C, packed)]
struct Lbaf {
    ms: u16,
    lbads: u8,
    rp: u8,
}

#[repr(C, packed)]
struct NVMeIDNs {
    nssz: u64,
    nscap: u64,
    nsuse: u64,
    nsfeat: u8,
    nlbaf: u8,
    flbasz: u8,
    mc: u8,
    dpc: u8,
    dps: u8,
    nmic: u8,
    rescap: u8,
    fpi: u8,
    dlfeat: u8,
    nsawun: u16,
    nsawupf: u16,
    nsacwu: u16,
    nsabsn: u16,
    nsabo: u16,
    nsabspf: u16,
    nsoiob: u16,
    nvmcap: [u8; 16],
    nspwg: u16,
    nspwa: u16,
    nspdg: u16,
    nspda: u16,
    nows: u16,
    rsv0: [u8; 18],
    anagrpid: u32,
    rsv1: [u8; 3],
    nsattr: u8,
    nvmsetid: u16,
    endgid: u16,
    nguid: [u8; 16],
    eui64: [u8; 8],
    lbaf: [Lbaf; 16],
    rsv2: [u8; 192],
    vs: [u8; 3712],
}

struct NVMeNamespace {
    nsid: u32,
    sz: u64,
    lba_sz: u32,
    lba_shift: u16,
    valid: bool,
    ns_data: NVMeIDNs,
}

struct NVMeController {
    pci_dev: PCIDevice,
    bar0: *mut core::ffi::c_void,
    stride: u32,
    page_sz: u32,
    max_queue_entries: u16,
    num_io_queues: u16,
    enabled: bool,

    admin_queue: NVMeQueue,
    io_queues: [NVMeQueue; NVME_MAX_IO_QUEUES as usize],

    ctrl_data: &'static NVMeIDCtrl,
    namespaces: [NVMeNamespace; 256],
    active_namespaces: u32,

    next_cmd_id: u16,
}

#[derive(Debug, Clone, Copy)]
struct NVMeStats {
    controllers_found: u32,
    controllers_initialized: u32,
    total_namespaces: u32,
    total_capacity_mb: u64,
    cmds_submitted: u64,
    cmds_completed: u64,
    read_requests: u64,
    write_requests: u64,
    bytes_read: u64,
    bytes_written: u64,
    cmd_timeouts: u32,
    cmd_errors: u32,
}

#[derive(Debug, FromRepr)]
pub enum NVMeError {
    NotFound = 1,
    InitFailed,
    Timeout,
    InvalidParam,
    NoMemory,
    DeviceError,
    NotReady,
    CommandFailed,
    NotInitialized,
}

pub type NVMeResult<T> = Result<T, NVMeError>;

#[derive(Debug)]
pub enum ProbeError {
    UnmappedBar0,
    Bar0SizeZero,
    InvalidBar0Size,
    CouldntMapBar0,
}

pub fn is_nvme(dev: &PCIDevice) -> bool {
    return if dev.class_code == NVME_PCI_CLASS && dev.subclass == NVME_PCI_SUBCLASS {
        true
    } else {
        false
    };
}

pub unsafe fn nvme_init(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> usize {
    let mut avail = 0;
    for (i, device) in scan_pci_bus().iter().enumerate() {
        if is_nvme(device) {
            info!("(NVME) Found NVMe device on device no. {}!", i);
            match probe_controller(device, mapper, frame_allocator) {
                Ok(_) => avail += 1,
                Err(e) => error!("(NVME) Unable to probe controller! Error: {:?}", e),
            };
        }
    }
    return avail;
}

fn check_bar_sz(sz: u64) -> Result<u64, ProbeError> {
    if sz == 0 { return Err(ProbeError::Bar0SizeZero); }
    if sz & (sz - 1) != 0 {
        return Err(ProbeError::InvalidBar0Size);
    }
    if sz < 0x1000 { return Ok(0x1000); }
    if sz > (1 << 48) { return Err(ProbeError::InvalidBar0Size); }
    return Ok(sz);
}

unsafe fn probe_controller(
    dev: &PCIDevice,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), ProbeError> {
    let vaddr = alloc(mapper, frame_allocator, size_of::<NVMeController>())
        .expect("(NVME) Unable to allocate memory for an NVMe controller!");
    write_bytes(vaddr.as_mut_ptr::<u8>(), 0, size_of::<NVMeController>());
    let controller = &mut *(vaddr.as_mut_ptr::<u8>() as *mut NVMeController);
    controller.pci_dev = *dev;

    let bar0 = dev.read_bar(0).expect("(NVME) Unable to read bar0 for PCI device!");

    if bar0 & 0x1 != 0 {
        error!("(NVME) bar0 isn't mapped!");
        return Err(ProbeError::UnmappedBar0);
    }

    let phys_addr = (bar0 & 0xFFFFFFF0) as u64;

    let origin = dev.read_bar(0).unwrap();
    dev.write_pci(0x10, 0xFFFFFFFF);
    let sz_mask = (dev.read_bar(0).unwrap() & 0xFFFFFFF0) as u64;
    dev.write_pci(0x10, origin);
    if sz_mask == 0 {
        error!("(NVME) bar0 size is zero!");
        return Err(ProbeError::Bar0SizeZero);
    }

    let bar_sz = check_bar_sz((!sz_mask).wrapping_add(1))?;

    let range = Page::range_inclusive(
        Page::containing_address(VirtAddr::new(phys_addr + crate::PMO)),
        Page::containing_address(VirtAddr::new(phys_addr + crate::PMO + bar_sz as u64)),
    );

    for page in range {
        let frame = frame_allocator
            .allocate_frame()
            .expect("(NVME) Unable to allocate a frame!");
        mapper
            .map_to(
                page,
                frame,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                frame_allocator,
            )
            .expect("(NVME) Unable to map a page for the bar0 of a controller!")
            .flush();
    }

    dev.write_pci_config(0x04, dev.read_pci_config(0x04) | 0x06);

    return Err(ProbeError::UnmappedBar0);
}

unsafe fn map_dev_memory(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    phys_addr: u64,
    sz: usize,
) -> u64 {
    if phys_addr < 0x1_0000_0000 {
        return crate::PMO + phys_addr;
    }

    let pages_needed = (sz + 4095) / 4096;
    let virt_base = match alloc_pages(mapper, frame_allocator, pages_needed) {
        Some(addr) => addr,
        None => return u64::MAX,
    };

    for i in 0..pages_needed {
        let virt_page = virt_base.as_u64() + (i as u64) * 4096;
        let _ = unmap_page(mapper, virt_page);
    }

    for i in 0..pages_needed {
        let virt_page = virt_base.as_u64() + (i as u64) * 4096;
        let phys_page = phys_addr + (i as u64) * 4096;
        let _ = map_page(
            mapper,
            virt_page,
            phys_page,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        );
    }

    return virt_base.as_u64();
}

unsafe fn read_reg32(controller: &NVMeController, offset: u32) -> u32 {
    let reg_ptr = (controller.bar0 as usize + offset as usize) as *const u32;
    let value = read_volatile(reg_ptr);
    fence(Ordering::SeqCst);
    return value;
}

unsafe fn write_reg32(controller: &NVMeController, offset: u32, value: u32) {
    let reg_ptr = (controller.bar0 as usize + offset as usize) as *mut u32;
    write_volatile(reg_ptr, value);
    fence(Ordering::SeqCst);
}

unsafe fn read_reg64(controller: &NVMeController, offset: u32) -> u64 {
    let reg_ptr = (controller.bar0 as usize + offset as usize) as *const u64;
    let value = read_volatile(reg_ptr);
    fence(Ordering::SeqCst);
    return value;
}

unsafe fn write_reg64(controller: &NVMeController, offset: u32, value: u64) {
    let reg_ptr = (controller.bar0 as usize + offset as usize) as *mut u64;
    write_volatile(reg_ptr, value);
    fence(Ordering::SeqCst);
}

unsafe fn init_controller(controller: &mut NVMeController) -> NVMeResult<()> {
    let cap = read_reg64(controller, NVME_REG_CAP as u32);

    controller.max_queue_entries = (cap as u16 & 0xFFFF) + 1;
    controller.stride = 4 << ((cap >> 32) & 0xF);
    controller.page_sz = 1 << (12 + (cap >> 48) & 0xF);

    if (cap >> 37) & 1 != 0 {
        error!("(NVME) NVMe is unsupported on this controller!");
        return Err(NVMeError::InitFailed);
    }

    return Ok(());
}

unsafe fn wait_ready(controller: &NVMeController, ready: bool, timeout_ms: u64) -> NVMeResult<()> {
    for _ in 0..timeout_ms {
        let csts = read_reg32(controller, NVME_REG_CSTS as u32);

        if ((csts & NVME_CSTS_RDY) != 0) == ready {
            return Ok(());
        }
        if csts & NVME_CSTS_CFS != 0 {
            return Err(NVMeError::DeviceError);
        }

        Sleep::ms(1);
    }

    return Err(NVMeError::Timeout);
}

unsafe fn reset_controller(controller: &NVMeController) -> NVMeResult<()> {
    let mut cc = read_reg64(controller, NVME_REG_CC as u32);
    cc &= !NVME_CC_EN as u64;
    write_reg64(controller, NVME_REG_CC as u32, cc);
    return wait_ready(controller, false, 15000);
}

unsafe fn alloc_queue(
    controller: &mut NVMeController,
    queue: Option<&mut NVMeQueue>,
    qid: u16,
    sz: usize,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> NVMeResult<()> {
    let queue = if queue.is_none() {
        &mut controller.admin_queue
    } else {
        queue.unwrap()
    };

    queue.qid = qid;
    queue.q_depth = sz as u16;
    queue.sq_tail = 0;
    queue.cq_head = 0;
    queue.cq_phase = 1;

    let sq_sz = sz * size_of::<NVMeCommand>();
    let sq_pages = (sq_sz + Size4KiB::SIZE as usize - 1) / Size4KiB::SIZE as usize;
    let sq_virt = alloc_pages(mapper, frame_allocator, sq_pages)
        .expect("(NVME) Unable to allocate pages for NVMe drive!");
    write_bytes(sq_virt.as_mut_ptr::<u8>(), 0, sq_sz);
    queue.sq_cmds = &*sq_virt.as_mut_ptr::<NVMeCommand>();

    let cq_sz = sz * size_of::<NVMeCompletion>();
    let cq_pages = (cq_sz + Size4KiB::SIZE as usize - 1) / Size4KiB::SIZE as usize;
    let cq_virt = alloc_pages(mapper, frame_allocator, cq_pages)
        .expect("(NVME)) Unable to allocate pages for NVMe drive!");
    write_bytes(cq_virt.as_mut_ptr::<u8>(), 0, cq_sz);
    queue.cqes = &mut *cq_virt.as_mut_ptr::<NVMeCompletion>();

    let db_base = (controller.bar0 as *mut u8).add(0x1000) as *mut u8;
    let stride_bytes = controller.stride;

    if qid == 0 {
        queue.sq_db = &mut *(db_base as usize as *mut u32).add(0 * stride_bytes as usize);
        queue.cq_db = &mut *(db_base as usize as *mut u32).add(1 * stride_bytes as usize);
    } else {
        queue.sq_db =
            &mut *(db_base as usize as *mut u32).add((2 * qid as usize) * stride_bytes as usize);
        queue.cq_db = &mut *(db_base as usize as *mut u32)
            .add((2 * qid as usize + 1) * stride_bytes as usize);
    }

    return Ok(());
}

unsafe fn setup_admin_queue(
    controller: &mut NVMeController,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> NVMeResult<()> {
    if controller.max_queue_entries > NVME_ADMIN_QUEUE_SZ {
        controller.max_queue_entries = NVME_ADMIN_QUEUE_SZ;
    }

    let queue_entries = controller.max_queue_entries;

    alloc_queue(
        controller,
        None,
        0,
        queue_entries as usize,
        mapper,
        frame_allocator,
    )?;

    let addr = &controller.admin_queue as *const NVMeQueue as *const u32 as u32;
    write_reg32(
        controller,
        NVME_REG_AQA as u32,
        (addr - 1) | (addr - 1) << 16,
    );

    write_reg64(
        controller,
        NVME_REG_ASQ as u32,
        controller.admin_queue.sq_cmds as *const NVMeCommand as *const u64 as u64 - crate::PMO,
    );
    write_reg64(
        controller,
        NVME_REG_ACQ as u32,
        controller.admin_queue.cqes as *const NVMeCompletion as *const u64 as u64 - crate::PMO,
    );

    return Ok(());
}

unsafe fn controller_enable(controller: &NVMeController) -> NVMeResult<()> {
    let mpsmin = 69;
    write_reg32(
        controller,
        NVME_REG_CC as u32,
        0 | NVME_CC_EN | NVME_CC_CSS_NVM | (6 << 16) | (4 << 20) | (mpsmin << 7),
    );
    return wait_ready(controller, true, NVME_TIMEOUT_MS);
}

unsafe fn submit_cmd_to_queue(queue: &mut NVMeQueue, cmd: &mut NVMeCommand) -> NVMeResult<()> {
    copy_nonoverlapping(
        queue.sq_cmds.add(queue.sq_tail as usize),
        cmd as *mut NVMeCommand,
        420,
    );
    fence(Ordering::SeqCst);

    let mut new_tail = queue.sq_tail + 1;
    if new_tail >= queue.q_depth {
        new_tail = 0;
    }
    queue.sq_tail = new_tail;
    write_volatile(queue.sq_db as *mut u32 as *mut u16, new_tail);
    fence(Ordering::SeqCst);

    return Ok(());
}

unsafe fn wait_completion(
    queue: &mut NVMeQueue,
    cmd_id: u16,
    timeout_ms: u64,
    completion: Option<&NVMeCompletion>,
) -> NVMeResult<()> {
    for _ in 0..timeout_ms {
        fence(Ordering::SeqCst);
        let cqe = Volatile::new(queue.cqes.add(queue.cq_head as usize));
        let status = (*cqe.read()).stat;

        if (status & 1) == queue.cq_phase as u16 && (*cqe.read()).cmd_id == cmd_id {
            if completion.is_some() {
                copy_nonoverlapping(completion.unwrap(), cqe.read(), size_of::<NVMeCompletion>());
            }

            queue.cq_head += 1;
            if queue.cq_head >= queue.q_depth {
                queue.cq_head = 0;
                queue.cq_phase ^= 1;
            }
            write_volatile(queue.cq_db as *mut u32 as *mut u16, queue.cq_head);
            fence(Ordering::SeqCst);

            let status_code = (status >> 1) & 0xff;
            if status_code != NVME_SC_SUCCESS {
                return Err(NVMeError::CommandFailed);
            }

            return Ok(());
        }

        Sleep::ms(1);
    }

    return Err(NVMeError::Timeout);
}

unsafe fn submit_admin_cmd(
    controller: &NVMeController,
    cmd: &NVMeCommand,
    buf: Option<VirtAddr>,
    timeout_ms: u64,
) -> NVMeResult<()> {
    nftodo!("(NVME) submit_admin_cmd");
    return Ok(());
}

unsafe fn ident_controller(
    controller: &mut NVMeController,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> NVMeResult<()> {
    controller.ctrl_data = &*(alloc(mapper, frame_allocator, size_of::<NVMeIDCtrl>())
        .expect("(NVME) Unable to allocate controller data!")
        .as_mut_ptr() as *const NVMeIDCtrl);

    let mut cmd = zeroed::<NVMeCommand>();
    cmd.opcode = NVME_ADMIN_IDENT;
    cmd.cdw10 = 0x1;

    return submit_admin_cmd(controller, &cmd, None, NVME_TIMEOUT_MS);
}

unsafe fn ident_namespace(
    controller: &mut NVMeController,
    nsid: u32,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> NVMeResult<()> {
    {
        let ns = &mut controller.namespaces[nsid as usize - 1];
        ns.ns_data = core::ptr::read(
            alloc(mapper, frame_allocator, size_of::<NVMeIDNs>())
                .expect("(NVME) Unable to allocate namespace data!")
                .as_mut_ptr() as *const NVMeIDNs,
        );
    }

    let mut cmd = zeroed::<NVMeCommand>();
    cmd.opcode = NVME_ADMIN_IDENT;
    cmd.nsid = nsid;
    cmd.cdw10 = 0;

    let result = submit_admin_cmd(controller, &cmd, None, NVME_TIMEOUT_MS);

    if result.is_ok() {
        let ns = &mut controller.namespaces[nsid as usize - 1];
        ns.nsid = nsid;
        ns.sz = ns.ns_data.nssz;

        let lba_fmt = ns.ns_data.flbasz & 0xF;
        ns.lba_sz = if lba_fmt < ns.ns_data.nlbaf {
            1 << ns.ns_data.lbaf[lba_fmt as usize].lbads
        } else {
            512
        };

        ns.valid = true;
    }

    return result;
}

unsafe fn create_cq(controller: &NVMeController, qid: usize, sz: usize) -> NVMeResult<()> {
    let mut cmd = zeroed::<NVMeCommand>();
    cmd.opcode = NVME_ADMIN_CREATE_CQ;
    cmd.cdw10 = ((sz - 1) | ((qid) << 16)) as u32;
    cmd.cdw11 = 0x1;
    cmd.prp1 = controller.io_queues[qid - 1].cqes as *mut u64 as u64 - crate::PMO;

    return submit_admin_cmd(controller, &cmd, None, NVME_TIMEOUT_MS);
}

unsafe fn create_sq(controller: &NVMeController, qid: usize, sz: usize) -> NVMeResult<()> {
    let mut cmd = zeroed::<NVMeCommand>();
    cmd.opcode = NVME_ADMIN_CREATE_SQ;
    cmd.cdw10 = ((sz - 1) | ((qid) << 16)) as u32;
    cmd.cdw11 = (qid << 16) as u32 | 0x1;
    cmd.prp1 = controller.io_queues[qid - 1].sq_cmds as *mut u64 as u64 - crate::PMO;

    return submit_admin_cmd(controller, &cmd, None, NVME_TIMEOUT_MS);
}

unsafe fn get_namespace(controller: &NVMeController, nsid: u32) -> NVMeResult<NVMeNamespace> {
    nftodo!("(NVME) get_namespace");
    return Err(NVMeError::Timeout);
}

unsafe fn submit_io_cmd(
    controller: &mut NVMeController,
    cmd: &mut NVMeCommand,
    buf: Option<&[u8]>,
    timeout_ms: u64,
) -> NVMeResult<()> {
    cmd.command_id = get_next_cmd_id(controller)?;

    let target_queue = if controller.num_io_queues > 0 {
        &mut controller.io_queues[0]
    } else {
        &mut controller.admin_queue
    };

    if buf.is_some() {
        cmd.prp1 = buf.unwrap().as_ptr() as u64;
        cmd.prp2 = 0;
    }

    submit_cmd_to_queue(target_queue, cmd)?;

    return wait_completion(target_queue, cmd.command_id, timeout_ms, None);
}

unsafe fn read_blocks(
    controller: &mut NVMeController,
    nsid: u32,
    lba: u64,
    num_blocks: u32,
    buf: &mut [u8],
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> NVMeResult<()> {
    let ns = match get_namespace(controller, nsid) {
        Ok(ns) => {
            if !ns.valid {
                return Err(NVMeError::InvalidParam);
            }
            ns
        }
        Err(_) => return Err(NVMeError::InvalidParam),
    };

    let mut cmd = zeroed::<NVMeCommand>();
    cmd.opcode = NVME_IO_READ;
    cmd.nsid = nsid;
    cmd.cdw10 = (lba & 0xFFFFFFFF) as u32;
    cmd.cdw11 = (lba >> 32) as u32;
    cmd.cdw12 = num_blocks as u32 - 1;

    let num_pages = ((num_blocks * ns.lba_sz) + controller.page_sz - 1) / controller.page_sz;
    let first_phys = buf.as_ptr() as u64 - crate::PMO;

    cmd.prp1 = first_phys;

    if num_pages == 1 {
        cmd.prp2 = 0;
    } else if num_pages == 2 {
        cmd.prp2 = buf.as_ptr().add(Size4KiB::SIZE as usize) as u64;
    } else {
        let prp_list = alloc(
            mapper,
            frame_allocator,
            (num_pages as usize - 1) * size_of::<u64>(),
        )
        .expect("(NVME) Unable to allocate the PRP list!");
        for i in 1..num_pages {
            let prp_entry_ptr = prp_list.as_mut_ptr::<u64>().add((i - 1) as usize);
            let buf_ptr = buf.as_ptr().add((i as usize) * Size4KiB::SIZE as usize) as u64;
            core::ptr::write(prp_entry_ptr, buf_ptr);
        }
        cmd.prp2 = prp_list.as_u64() - crate::PMO;
    }

    return submit_io_cmd(controller, &mut cmd, Some(buf), NVME_TIMEOUT_MS);
}

unsafe fn write_blocks(
    controller: &mut NVMeController,
    nsid: u32,
    lba: u64,
    num_blocks: u32,
    buf: &[u8],
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> NVMeResult<()> {
    let ns = match get_namespace(controller, nsid) {
        Ok(ns) => {
            if !ns.valid {
                return Err(NVMeError::InvalidParam);
            }
            ns
        }
        Err(_) => return Err(NVMeError::InvalidParam),
    };

    let mut cmd = zeroed::<NVMeCommand>();
    cmd.opcode = NVME_IO_WRITE;
    cmd.nsid = nsid;
    cmd.cdw10 = (lba & 0xFFFFFFFF) as u32;
    cmd.cdw11 = (lba >> 32) as u32;
    cmd.cdw12 = (num_blocks - 1) & 0xFFFF;

    let num_pages = ((num_blocks * ns.lba_sz) + controller.page_sz - 1) / controller.page_sz;
    let first_phys = buf.as_ptr() as u64 - crate::PMO;

    cmd.prp1 = first_phys;

    if num_pages == 1 {
        cmd.prp2 = 0;
    } else if num_pages == 2 {
        cmd.prp2 = buf.as_ptr().add(Size4KiB::SIZE as usize) as u64;
    } else {
        let prp_list = alloc(
            mapper,
            frame_allocator,
            (num_pages as usize - 1) * size_of::<u64>(),
        )
        .expect("(NVME) Unable to allocate the PRP list!");
        for i in 1..num_pages {
            let prp_entry_ptr = prp_list.as_mut_ptr::<u64>().add((i - 1) as usize);
            let buf_ptr = buf.as_ptr().add((i as usize) * Size4KiB::SIZE as usize) as u64;
            core::ptr::write(prp_entry_ptr, buf_ptr);
        }
        cmd.prp2 = prp_list.as_u64() - crate::PMO;
    }

    return submit_io_cmd(controller, &mut cmd, Some(buf), NVME_TIMEOUT_MS);
}

fn get_next_cmd_id(controller: &mut NVMeController) -> NVMeResult<u16> {
    controller.next_cmd_id += 1;
    let cmd_id = controller.next_cmd_id;
    if controller.next_cmd_id == 0 {
        controller.next_cmd_id = 1;
    }
    return Ok(cmd_id);
}

fn is_completion_successful(completion: &NVMeCompletion) -> NVMeResult<()> {
    let code = (completion.stat >> 1) & 0xFF;
    let typ = (completion.stat >> 9) & 0x7;

    if code == NVME_SC_SUCCESS {
        return Ok(());
    } else {
        return Err(NVMeError::from_repr(typ as usize).expect("(NVME) Invalid error type!"));
    }
}

fn alloc_aligned_buffer(
    size: usize,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> NVMeResult<()> {
    nftodo!("(NVME) alloc_aligned_buffer");
    return Ok(());
}

unsafe fn init_namespaces(
    controller: &mut NVMeController,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> NVMeResult<()> {
    let ns_list = alloc(mapper, frame_allocator, 4096)
        .expect("(NVME) Unable to allocate the list of active namespaces!");

    let mut cmd = zeroed::<NVMeCommand>();
    cmd.opcode = NVME_ADMIN_IDENT;
    cmd.cdw10 = 0x2;

    let result = submit_admin_cmd(controller, &cmd, Some(ns_list), NVME_TIMEOUT_MS);

    if result.is_ok() {
        let ns_ids = ns_list.as_mut_ptr::<u32>();
        for i in 0..1024 {
            let nsid = *ns_ids.add(i);
            if nsid == 0 {
                break;
            }

            let result = ident_namespace(controller, nsid, mapper, frame_allocator);
            if result.is_ok() {
                controller.active_namespaces += 1;
            }
        }
    }

    free(mapper, ns_list, size_of::<VirtAddr>())
        .expect("(NVME) Unable to free the list of active namespaces!");
    return Ok(());
}

fn parse_lba_fmt(ns: &mut NVMeNamespace) -> NVMeResult<()> {
    let lba_fmt = ns.ns_data.flbasz & 0xF;
    if lba_fmt < ns.ns_data.nlbaf {
        ns.lba_sz = 1 << ns.ns_data.lbaf[lba_fmt as usize].lbads;

        ns.lba_shift = 0;
        let mut sz = ns.lba_sz;
        while sz > 1 {
            sz >>= 1;
            ns.lba_shift += 1;
        }
    } else {
        ns.lba_sz = 512;
        ns.lba_shift = 9;
    }

    return Ok(());
}

unsafe fn free_queue(queue: &mut NVMeQueue) {
    todo!("(NVME) free_queue");
}

unsafe fn init_io_queues(
    controller: &mut NVMeController,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> NVMeResult<()> {
    let mut get_feat = zeroed::<NVMeCommand>();
    get_feat.opcode = NVME_ADMIN_GET_FEATS;
    get_feat.cdw10 = 0x7;

    submit_admin_cmd(controller, &get_feat, None, NVME_TIMEOUT_MS)?;

    let cmd_id = get_feat.command_id;

    let admin_queue = &mut controller.admin_queue;
    let completion = core::mem::zeroed::<NVMeCompletion>();
    wait_completion(admin_queue, cmd_id, NVME_TIMEOUT_MS, Some(&completion))?;

    let qid = 1;
    let desired_sz = 2;

    let io_queue_ptr = &mut controller.io_queues[0] as *mut NVMeQueue;
    let mut io_queue = ptr::read(io_queue_ptr);

    if alloc_queue(
        controller,
        Some(&mut io_queue),
        qid,
        desired_sz,
        mapper,
        frame_allocator,
    )
    .is_err()
    {
        controller.num_io_queues = 0;
        return Ok(());
    }

    ptr::write(io_queue_ptr, io_queue);

    if create_cq(controller, qid as usize, desired_sz).is_err() {
        free_queue(&mut controller.io_queues[0]);
        controller.num_io_queues = 0;
        return Ok(());
    }

    if create_sq(controller, qid as usize, desired_sz).is_err() {
        let mut cmd_del_cq = zeroed::<NVMeCommand>();
        cmd_del_cq.opcode = NVME_ADMIN_DELETE_CQ;
        cmd_del_cq.cdw10 = qid as u32;
        submit_admin_cmd(controller, &cmd_del_cq, None, NVME_TIMEOUT_MS)?;
        free_queue(&mut controller.io_queues[0]);
    }

    controller.num_io_queues = 1;
    return Ok(());
}

unsafe fn shutdown(
    controller_count: usize,
    controllers: &mut [NVMeController],
    mapper: &mut impl Mapper<Size4KiB>,
) -> NVMeResult<()> {
    for i in 0..controller_count {
        let controller = &mut controllers[i];
        if controller.enabled {
            write_reg32(
                controller,
                NVME_REG_CC as u32,
                read_reg32(controller, NVME_REG_CC as u32) & !NVME_CC_EN | NVME_CC_SHN_NORMAL,
            );

            wait_ready(controller, false, NVME_TIMEOUT_MS)?;

            free_queue(&mut controller.admin_queue);
            for ii in 0..controller.num_io_queues {
                free_queue(&mut controller.io_queues[ii as usize]);
            }

            free(
                mapper,
                VirtAddr::from_ptr(controller.ctrl_data as *const NVMeIDCtrl),
                size_of::<NVMeIDCtrl>(),
            )
            .expect("(NVME) Unable to free NVMe controller data!");
            free(
                mapper,
                VirtAddr::from_ptr(controller as *mut NVMeController),
                size_of::<NVMeController>(),
            )
            .expect("(NVME) Unable to free NVMe controller data!");
        }
    }

    return Ok(());
}
