// This code was adapted from VoltagedDebunked/osdev-nvme on GitHub.

use crate::{
    allocator::{alloc, alloc_pages, free, map_page},
    error, info, nftodo,
    pci::{self, PCIDevice},
    sleep::Sleep,
    warning,
};
use alloc::vec::*;
use core::{
    intrinsics::compare_bytes,
    ptr::{copy_nonoverlapping, write_bytes},
    sync::atomic::{AtomicUsize, Ordering, fence},
};
use volatile::Volatile;
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageSize, PageTableFlags, Size4KiB, mapper::MapToError,
    },
};

const NVME_PCI_CLASS: u32 = 0x01;
const NVME_PCI_SUBCLASS: u32 = 0x08;
const NVME_PCI_PROG_IF: u8 = 0x02;

const NVME_REG_CAP: u8 = 0x00;
const NVME_REG_VS: u8 = 0x08;
const NVME_REG_CC: u8 = 0x14;
const NVME_REG_CSTS: u8 = 0x1C;
const NVME_REG_AQA: u8 = 0x24;
const NVME_REG_ASQ: u8 = 0x28;
const NVME_REG_ACQ: u8 = 0x30;

const NVME_CAP_MQES_MASK: u64 = 0xFFFF;
const NVME_CAP_CQR: u64 = 1 << 16;
const NVME_CAP_DSTRD_MASK: u64 = 0xF << 32;
const NVME_CAP_CSS_MASK: u64 = 0xFF << 37;
const NVME_CAP_MPSMIN_MASK: u64 = 0xF << 48;
const NVME_CAP_MPSMAX_MASK: u64 = 0xF << 52;

const NVME_CC_EN: u64 = 1 << 0;
const NVME_CC_CSS_NVM: u64 = 0 << 4;
const NVME_CC_SHN_NORMAL: u64 = 1 << 14;

const NVME_CSTS_RDY: u8 = 1 << 0;
const NVME_CSTS_CFS: u8 = 1 << 1;

const NVME_MAX_IO_QUEUES: usize = 8;

const NVME_RESET_TIMEOUT_MS: u32 = 5000;
const NVME_ENABLE_TIMEOUT_MS: u32 = 1000;
const NVME_COMMAND_TIMEOUT_MS: u32 = 30000;
const NVME_SHUTDOWN_TIMEOUT_MS: u32 = 5000;

const NVME_ADMIN_QUEUE_SIZE: u16 = 64;
const NVME_ADMIN_IDENTIFY: u8 = 0x06;
const NVME_ADMIN_DELETE_CQ: u8 = 0x04;
const NVME_ADMIN_CREATE_CQ: u8 = 0x05;
const NVME_ADMIN_DELETE_SQ: u8 = 0x00;
const NVME_ADMIN_CREATE_SQ: u8 = 0x01;
const NVME_ADMIN_GET_FEATURES: u8 = 0x0A;

const NVME_IO_FLUSH: u8 = 0x00;
const NVME_IO_WRITE: u8 = 0x01;
const NVME_IO_READ: u8 = 0x02;

const NVME_SC_SUCCESS: u8 = 0x00;

#[repr(C, packed)]
struct NVMeCommand {
    opcode: u8,
    flags: u8,
    command_id: u16,
    nsid: u32,
    rsv0: u64,
    metadata: u64,
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
    command_id: u16,
    status: u16,
}

#[derive(Clone)]
struct NVMeQueue {
    sq_cmds: *mut NVMeCommand,
    cqes: *mut NVMeCompletion,
    sq_db: *mut u32,
    cq_db: *mut u32,
    sq_tail: u16,
    cq_head: u16,
    qid: u16,
    q_depth: u16,
    cq_phase: u8,
    sqe_shift: u8,
    cqe_shift: u8,
}

pub struct NVMeController {
    pci_dev: *mut PCIDevice,
    bar0: u32,
    stride: u32,
    page_size: u32,
    max_queue_entries: u16,
    num_io_queues: u16,
    enabled: bool,

    admin_queue: NVMeQueue,
    io_queues: [NVMeQueue; NVME_MAX_IO_QUEUES],

    ctrl_data: *mut NVMeIDCtrl,
    namespaces: [NVMeNamespace; 256],
    num_namespaces: u32,

    next_cmd_id: u16,
}

pub type NVMeResult<T> = Result<T, NVMeError>;

impl NVMeController {
    unsafe fn read_reg32(&self, offset: u32) -> Option<u32> {
        let reg = Volatile::new(
            match (*self.pci_dev).read_bar(0) {
                Some(b) => b.0,
                None => {
                    error!("(NVME) Unable to read Bar #0 of PCI(e) device!");
                    return None;
                }
            } + offset,
        );
        let val = reg.read();
        fence(Ordering::SeqCst);
        return Some(val);
    }

    unsafe fn write_reg32(&self, offset: u32, val: u32) -> Option<u32> {
        let mut reg = Volatile::new(
            match (*self.pci_dev).read_bar(0) {
                Some(b) => b.0,
                None => {
                    error!("(NVME) Unable to read Bar #0 of PCI(e) device!");
                    return None;
                }
            } + offset,
        );
        reg.write(val);
        let val = reg.read();
        fence(Ordering::SeqCst);
        return Some(val);
    }

    unsafe fn read_reg64(&self, offset: u32) -> Option<u64> {
        let reg_addr = match (*self.pci_dev).read_bar(0) {
            Some(b) => b.0 + offset,
            None => {
                error!("(NVME) Unable to read Bar #0 of PCI(e) device!");
                return None;
            }
        };

        let reg_ptr = reg_addr as *const u64;
        let reg = Volatile::new(reg_ptr);

        let val = reg.read();
        fence(Ordering::SeqCst);
        return Some(*val);
    }

    unsafe fn write_reg64(&self, offset: u32, val: u64) -> Option<u64> {
        let reg_addr = match (*self.pci_dev).read_bar(0) {
            Some(b) => b.0 + offset,
            None => {
                error!("(NVME) Unable to read Bar #0 of PCI(e) device!");
                return None;
            }
        };

        let reg_ptr = reg_addr as *mut u64;
        let mut reg = Volatile::new(reg_ptr);

        reg.write(val as *mut _);
        let val = reg.read();
        fence(Ordering::SeqCst);
        return Some(*val);
    }

    unsafe fn init(&mut self) -> NVMeResult<()> {
        let cap = match self.read_reg64(NVME_REG_CAP as u32) {
            Some(c) => c,
            None => {
                error!("(NVME) Unable to read capabilities register!");
                return Err(NVMeError::RWRegError);
            }
        };
        self.max_queue_entries = cap as u16 + 1;
        self.stride = 4 << (cap >> 32) as u8;

        let mpsmin = (cap >> 48) as u8;
        self.page_size = 1 << (12 + mpsmin);

        let nvm_supported = (cap >> 37) & 1;

        if nvm_supported == 0 {
            return Err(NVMeError::InitFailed);
        }

        let mut admin_queue_size = self.max_queue_entries;
        if admin_queue_size > NVME_ADMIN_QUEUE_SIZE {
            admin_queue_size = NVME_ADMIN_QUEUE_SIZE;
        }

        //self.alloc_queue(0, admin_queue_size)?;

        self.write_reg32(
            NVME_REG_AQA as u32,
            admin_queue_size as u32 - 1 | (admin_queue_size as u32 - 1) << 16,
        );

        self.write_reg64(
            NVME_REG_ASQ as u32,
            self.admin_queue.sq_cmds.sub(unsafe { crate::PMO as usize }) as u64,
        );
        self.write_reg64(
            NVME_REG_ACQ as u32,
            self.admin_queue.cqes.sub(unsafe { crate::PMO as usize }) as u64,
        );

        let cc = match self.read_reg32(NVME_REG_CC as u32) {
            Some(c) => c,
            None => {
                error!("(NVME) Unable to read CC register!");
                return Err(NVMeError::RWRegError);
            }
        };

        self.write_reg32(
            (0 | NVME_CC_EN | NVME_CC_CSS_NVM | 6 << 16 | 4 << 20 | (mpsmin as u64) << 7) as u32,
            cc,
        );

        self.wait_ready(true, NVME_ENABLE_TIMEOUT_MS);

        return Ok(());
    }

    unsafe fn wait_ready(&self, ready: bool, timeout_ms: u32) -> NVMeResult<()> {
        for _ in 0..timeout_ms {
            let csts = match self.read_reg32(NVME_REG_CSTS as u32) {
                Some(c) => c,
                None => {
                    error!("(NVME) Unable to read CSTS register!");
                    return Err(NVMeError::RWRegError);
                }
            };
            let is_ready = (csts & NVME_CSTS_RDY as u32) != 0;

            if is_ready == ready {
                return Ok(());
            }

            if csts & NVME_CSTS_CFS as u32 != 0 {
                return Err(NVMeError::DeviceError);
            }

            Sleep::ms(1);
        }

        return Err(NVMeError::Timeout);
    }

    unsafe fn reset(&self) -> NVMeResult<()> {
        self.write_reg32(
            NVME_REG_CC as u32,
            match self.read_reg32(NVME_REG_CC as u32) {
                Some(c) => c & !NVME_CC_EN as u32,
                None => {
                    error!("(NVME) Unable to read CC register!");
                    return Err(NVMeError::RWRegError);
                }
            },
        );
        return self.wait_ready(false, NVME_RESET_TIMEOUT_MS);
    }

    unsafe fn alloc_queue(
        &self,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        queue: *mut NVMeQueue,
        qid: u16,
        size: u16,
    ) -> NVMeResult<()> {
        (*queue).qid = qid;
        (*queue).q_depth = size;
        (*queue).sq_tail = 0;
        (*queue).cq_head = 0;
        (*queue).cq_phase = 0;

        let sq_size = size as usize * size_of::<NVMeCommand>();
        let sq_pages = (sq_size + Size4KiB::SIZE as usize - 1) / Size4KiB::SIZE as usize;
        let sq_virt = match alloc_pages(mapper, frame_allocator, sq_pages) {
            Some(v) => v,
            None => {
                error!("(NVME) Unable to allocate {} pages!", sq_pages);
                return Err(NVMeError::OutOfMemory);
            }
        };
        write_bytes(sq_virt.as_mut_ptr::<u8>(), 0, sq_size);
        (*queue).sq_cmds = sq_virt.as_mut_ptr();

        let cq_size = size as usize * size_of::<NVMeCommand>();
        let cq_pages = (cq_size + Size4KiB::SIZE as usize - 1) / Size4KiB::SIZE as usize;
        let cq_virt = match alloc_pages(mapper, frame_allocator, cq_pages) {
            Some(v) => v,
            None => {
                error!("(NVME) Unable to allocate {} pages!", cq_pages);
                return Err(NVMeError::OutOfMemory);
            }
        };
        write_bytes(cq_virt.as_mut_ptr::<u8>(), 0, cq_size);
        (*queue).cqes = cq_virt.as_mut_ptr();

        let doorbell_base = (self.bar0 as *mut u8).add(0x1000);

        if qid == 0 {
            (*queue).sq_db = doorbell_base as *mut u32;
            (*queue).cq_db = (doorbell_base as *mut u32).add(self.stride as usize);
        } else {
            (*queue).sq_db =
                (doorbell_base as *mut u32).add((2 * qid as usize) * self.stride as usize);
            (*queue).cq_db =
                (doorbell_base as *mut u32).add((2 * qid as usize + 1) * self.stride as usize);
        }

        return Ok(());
    }

    unsafe fn wait_completion(
        &self,
        queue: *mut NVMeQueue,
        cmd_id: u16,
        timeout_ms: u32,
        completion: *mut NVMeCompletion,
    ) -> NVMeResult<()> {
        let mut timeout_loops = timeout_ms * 1000;

        while timeout_loops > 0 {
            fence(Ordering::SeqCst);

            let cqe: Volatile<*mut NVMeCompletion> =
                Volatile::new((*queue).cqes.add((*queue).cq_head as usize));
            let status = (*cqe.read()).status;

            let phase_match = (status & 1) == (*queue).cq_phase as u16;
            let cmd_match = (*cqe.read()).command_id == cmd_id;

            if phase_match && cmd_match {
                if !completion.is_null() {
                    copy_nonoverlapping(completion, cqe.read(), size_of::<NVMeCompletion>());
                }

                (*queue).cq_head += 1;
                if (*queue).cq_head >= (*queue).q_depth {
                    (*queue).cq_head = 0;
                    (*queue).cq_phase ^= 1;
                }

                (*queue).cq_db = (*queue).cq_head as *mut u32;
                let _ = AtomicUsize::new(0);

                let status_code = status as u8 >> 1;
                if status_code != NVME_SC_SUCCESS {
                    return Err(NVMeError::CommandFailed);
                }

                return Ok(());
            }

            timeout_loops -= 1;
        }

        return Err(NVMeError::Timeout);
    }

    unsafe fn submit_admin_cmd(
        &mut self,
        cmd: *mut NVMeCommand,
        buffer: *mut u8,
        timeout_ms: u32,
    ) -> NVMeResult<()> {
        (*cmd).command_id = match self.get_next_cmd_id() {
            Ok(i) => i,
            Err(e) => {
                error!("(NVME) Unable to get next cmd id! Error: {:?}", e);
                return Err(e);
            }
        };

        if !buffer.is_null() {
            (*cmd).prp1 = buffer as u64 + crate::PMO;
            (*cmd).prp2 = 0;
        } else {
            (*cmd).prp1 = 0;
            (*cmd).prp2 = 0;
        }

        let aq = &mut self.admin_queue as *mut _;
        match self.submit_cmd_to_queue(aq, cmd) {
            Ok(_) => {}
            Err(e) => {
                error!("(NVME) Unable to submit cmd to admin queue! Error: {:?}", e);
                return Err(e);
            }
        }

        return self.wait_completion(aq, (*cmd).command_id, timeout_ms, 0 as *mut _);
    }

    unsafe fn identify(
        &mut self,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> NVMeResult<()> {
        self.ctrl_data = match alloc(mapper, frame_allocator, size_of::<NVMeIDCtrl>()) {
            Some(v) => v.as_mut_ptr(),
            None => {
                error!(
                    "(NVME) Unable to allocate {} bytes to identify controller!",
                    size_of::<NVMeIDCtrl>()
                );
                return Err(NVMeError::OutOfMemory);
            }
        };

        let mut cmd = NVMeCommand {
            opcode: NVME_ADMIN_IDENTIFY,
            flags: 0,
            command_id: 0,
            nsid: 0,
            rsv0: 0,
            metadata: 0,
            prp1: 0,
            prp2: 0,
            cdw10: 0x1,
            cdw11: 0,
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
        };

        return self.submit_admin_cmd(
            &mut cmd as *mut NVMeCommand,
            self.ctrl_data as *mut u8,
            NVME_COMMAND_TIMEOUT_MS,
        );
    }

    unsafe fn identify_namespace(
        &mut self,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        nsid: u32,
    ) -> NVMeResult<()> {
        let nsd = {
            let ns = match self.namespaces.iter_mut().nth(nsid as usize - 1) {
                Some(n) => n,
                None => {
                    error!("(NVME) Unable to get namespace #{}!", nsid);
                    return Err(NVMeError::NotFound);
                }
            };
            ns.ns_data = match alloc(mapper, frame_allocator, size_of::<NVMeIDNS>()) {
                Some(v) => v.as_mut_ptr(),
                None => {
                    error!(
                        "(NVME) Unable to allocate {} bytes to identify a namespace!",
                        size_of::<NVMeIDNS>()
                    );
                    return Err(NVMeError::OutOfMemory);
                }
            };
            ns.ns_data
        };

        let mut cmd = NVMeCommand {
            opcode: NVME_ADMIN_IDENTIFY,
            flags: 0,
            command_id: 0,
            nsid,
            rsv0: 0,
            metadata: 0,
            prp1: 0,
            prp2: 0,
            cdw10: 0,
            cdw11: 0,
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
        };

        self.submit_admin_cmd(&mut cmd as *mut _, nsd as *mut _, NVME_COMMAND_TIMEOUT_MS)?;

        let ns = &mut self.namespaces[nsid as usize - 1];
        ns.nsid = nsid;
        ns.size = (*ns.ns_data).nsze;

        let lba_format = (*ns.ns_data).flbas as u8;
        if lba_format < (*ns.ns_data).nlbaf {
            ns.lba_size = 1 << (*ns.ns_data).lbaf[lba_format as usize].lbads;
        } else {
            ns.lba_size = 512;
        }

        ns.valid = true;

        return Ok(());
    }

    unsafe fn create_cq(&mut self, qid: u16, size: u16) -> NVMeResult<()> {
        let mut cmd = NVMeCommand {
            opcode: NVME_ADMIN_CREATE_CQ,
            flags: 0,
            command_id: 0,
            nsid: 0,
            rsv0: 0,
            metadata: 0,
            prp1: self.io_queues[qid as usize - 1].cqes as u64 - crate::PMO,
            prp2: 0,
            cdw10: size as u32 - 1 | (qid as u32) << 16,
            cdw11: 0x1,
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
        };

        return self.submit_admin_cmd(
            &mut cmd as *mut NVMeCommand,
            0 as *mut _,
            NVME_COMMAND_TIMEOUT_MS,
        );
    }

    unsafe fn create_sq(&mut self, qid: u16, size: u16) -> NVMeResult<()> {
        let mut cmd = NVMeCommand {
            opcode: NVME_ADMIN_CREATE_SQ,
            flags: 0,
            command_id: 0,
            nsid: 0,
            rsv0: 0,
            metadata: 0,
            prp1: self.io_queues[qid as usize - 1].sq_cmds as u64 - crate::PMO,
            prp2: 0,
            cdw10: size as u32 - 1 | (qid as u32) << 16,
            cdw11: (qid as u32) << 16 | 0x1,
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
        };

        return self.submit_admin_cmd(
            &mut cmd as *mut NVMeCommand,
            0 as *mut _,
            NVME_COMMAND_TIMEOUT_MS,
        );
    }

    unsafe fn get_namespace(&mut self, nsid: u32) -> NVMeResult<&mut NVMeNamespace> {
        if nsid == 0 || nsid > 256 {
            return Err(NVMeError::NamespaceNotFound);
        }

        let ns = &mut self.namespaces[nsid as usize - 1];
        return if ns.valid {
            Ok(ns)
        } else {
            Err(NVMeError::NamespaceNotFound)
        };
    }

    unsafe fn get_next_cmd_id(&mut self) -> NVMeResult<u16> {
        let cmd_id = self.next_cmd_id + 1;
        if self.next_cmd_id == 0 {
            self.next_cmd_id = 1;
        }
        return Ok(cmd_id);
    }

    unsafe fn submit_cmd_to_queue(
        &mut self,
        queue: *mut NVMeQueue,
        cmd: *mut NVMeCommand,
    ) -> NVMeResult<()> {
        copy_nonoverlapping(
            &mut (*queue).sq_tail as *mut u16 as *mut _,
            cmd,
            size_of::<NVMeCommand>(),
        );

        let _ = AtomicUsize::new(0);

        (*queue).sq_tail = if (*queue).sq_tail >= (*queue).q_depth {
            (*queue).sq_tail + 1
        } else {
            0
        };
        (*queue).sq_db = (*queue).sq_tail as *mut u16 as *mut _;

        let _ = AtomicUsize::new(0);

        return Ok(());
    }

    unsafe fn submit_io_cmd(
        &mut self,
        cmd: *mut NVMeCommand,
        buffer: *mut u8,
        timeout_ms: u32,
    ) -> NVMeResult<()> {
        let target_queue = if self.num_io_queues > 0 {
            &mut self.io_queues[0] as *mut _
        } else {
            &mut self.admin_queue as *mut _
        };

        (*cmd).command_id = self.get_next_cmd_id()?;

        if !buffer.is_null() {
            (*cmd).prp1 = buffer as u64 - crate::PMO;
            (*cmd).prp2 = 0;
        }

        self.submit_cmd_to_queue(target_queue, cmd)?;

        return self.wait_completion(target_queue, (*cmd).command_id, timeout_ms, 0 as *mut _);
    }

    unsafe fn read_blocks(
        &mut self,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        nsid: u32,
        lba: u64,
        num_blocks: u32,
        buffer: *mut u8,
    ) -> NVMeResult<()> {
        let ns = match self.get_namespace(nsid) {
            Ok(n) => n,
            Err(e) => {
                error!("(NVME) Unable to get namespace #{}! Error: {:?}", nsid, e);
                return Err(e);
            }
        };

        if !ns.valid {
            return Err(NVMeError::InvalidParam);
        }

        let mut cmd = NVMeCommand {
            opcode: NVME_IO_READ,
            flags: 0,
            command_id: 0,
            nsid: nsid,
            rsv0: 0,
            metadata: 0,
            prp1: 0,
            prp2: 0,
            cdw10: lba as u32,
            cdw11: (lba >> 32) as u32,
            cdw12: num_blocks - 1,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
        };

        let transfer_size = num_blocks * ns.lba_size;
        let num_pages = (transfer_size + self.page_size - 1) / self.page_size;
        let first_phys = buffer.sub(crate::PMO as usize);
        cmd.prp1 = first_phys as u64;

        if num_pages == 1 {
            cmd.prp2 = 0;
        } else if num_pages == 2 {
            cmd.prp2 = buffer as u64 + self.page_size as u64 - crate::PMO;
        } else {
            let prp_list = match alloc(
                mapper,
                frame_allocator,
                (num_pages as usize - 1) * size_of::<u64>(),
            ) {
                Some(v) => v.as_mut_ptr::<u32>(),
                None => {
                    error!(
                        "(NVME) Unable to allocate {} bytes to read blocks!",
                        (num_pages as usize - 1) * size_of::<u64>()
                    );
                    return Err(NVMeError::OutOfMemory);
                }
            };
            for i in 0..num_pages {
                *prp_list.add(i as usize - 1) = buffer as u32 + i as u32 * self.page_size;
            }

            cmd.prp2 = prp_list as u64 - crate::PMO;
        }

        return self.submit_io_cmd(&mut cmd as *mut _, buffer, NVME_COMMAND_TIMEOUT_MS);
    }

    unsafe fn write_blocks(
        &mut self,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        nsid: u32,
        lba: u64,
        num_blocks: u32,
        buffer: *mut u8,
    ) -> NVMeResult<()> {
        let ns = match self.get_namespace(nsid) {
            Ok(n) => n,
            Err(e) => {
                error!("(NVME) Unable to get namespace #{}! Error: {:?}", nsid, e);
                return Err(e);
            }
        };

        if !ns.valid {
            return Err(NVMeError::InvalidParam);
        }

        let mut cmd = NVMeCommand {
            opcode: NVME_IO_READ,
            flags: 0,
            command_id: 0,
            nsid: nsid,
            rsv0: 0,
            metadata: 0,
            prp1: 0,
            prp2: 0,
            cdw10: lba as u32,
            cdw11: (lba >> 32) as u32,
            cdw12: (num_blocks - 1) & 0xFFFF,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
        };

        let transfer_size = num_blocks * ns.lba_size;
        let num_pages = (transfer_size + self.page_size - 1) / self.page_size;
        let first_phys = buffer.sub(crate::PMO as usize);
        cmd.prp1 = first_phys as u64;

        if num_pages == 1 {
            cmd.prp2 = 0;
        } else if num_pages == 2 {
            cmd.prp2 = buffer as u64 + self.page_size as u64 - crate::PMO;
        } else {
            let prp_list = match alloc(
                mapper,
                frame_allocator,
                (num_pages as usize - 1) * size_of::<u64>(),
            ) {
                Some(v) => v.as_mut_ptr::<u32>(),
                None => {
                    error!(
                        "(NVME) Unable to allocate {} bytes to read blocks!",
                        (num_pages as usize - 1) * size_of::<u64>()
                    );
                    return Err(NVMeError::OutOfMemory);
                }
            };
            for i in 0..num_pages {
                *prp_list.add(i as usize - 1) = buffer as u32 + i as u32 * self.page_size;
            }

            cmd.prp2 = prp_list as u64 - crate::PMO;
        }

        return self.submit_io_cmd(&mut cmd as *mut _, buffer, NVME_COMMAND_TIMEOUT_MS);
    }

    unsafe fn init_namespaces(
        &mut self,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> NVMeResult<()> {
        let ns_list = match alloc(mapper, frame_allocator, 4096) {
            Some(v) => v.as_mut_ptr::<u8>(),
            None => {
                error!("(NVME) Unable to allocate 4096 bytes to init namespaces!");
                return Err(NVMeError::InitFailed);
            }
        };

        let mut cmd = NVMeCommand {
            opcode: NVME_ADMIN_IDENTIFY,
            flags: 0,
            command_id: 0,
            nsid: 0,
            rsv0: 0,
            metadata: 0,
            prp1: 0,
            prp2: 0,
            cdw10: 0x2,
            cdw11: 0,
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
        };

        self.submit_admin_cmd(&mut cmd as *mut _, ns_list, NVME_COMMAND_TIMEOUT_MS)?;

        for i in 0..1024 {
            let nsid = ns_list.add(i);
            if nsid.is_null() {
                break;
            }

            self.identify_namespace(mapper, frame_allocator, nsid as u32)?;
            self.num_namespaces += 1;
        }

        match free(mapper, VirtAddr::new(ns_list as u64), 4096) {
            Ok(_) => {}
            Err(e) => warning!(
                "(NVME) Unable to free data! Error: {:?}. Assuming okay...",
                e
            ),
        }

        return Ok(());
    }

    unsafe fn init_io_queues(
        &mut self,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> NVMeResult<()> {
        let mut get_feat = NVMeCommand {
            opcode: NVME_ADMIN_GET_FEATURES,
            flags: 0,
            command_id: 0,
            nsid: 0,
            rsv0: 0,
            metadata: 0,
            prp1: 0,
            prp2: 0,
            cdw10: 0x7,
            cdw11: 0,
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
        };

        self.submit_admin_cmd(
            &mut get_feat as *mut _,
            0 as *mut u8,
            NVME_COMMAND_TIMEOUT_MS,
        )?;

        let mut completion: NVMeCompletion = NVMeCompletion {
            result: 0,
            rsv0: 0,
            sq_head: 0,
            sq_id: 0,
            command_id: 0,
            status: 0,
        };
        self.wait_completion(
            &mut self.admin_queue.clone() as *mut _,
            get_feat.command_id,
            NVME_COMMAND_TIMEOUT_MS,
            &mut completion as *mut _,
        )?;

        let max_sq = completion.result as u16 + 1;
        let max_cq = (completion.result >> 16) as u16 + 1;

        let queue = &mut self.io_queues[0] as *mut _;
        match self.alloc_queue(mapper, frame_allocator, queue, 1, 2) {
            Ok(_) => {}
            Err(e) => {
                warning!(
                    "(NVME) Unable to allocate queue! Error: {:?}. Falling back to admin queues only...",
                    e
                );

                self.num_io_queues = 0;
                return Ok(());
            }
        }

        match self.create_cq(1, 2) {
            Ok(_) => {}
            Err(e) => {
                warning!(
                    "(NVME) Unable to create completion queue! Error: {:?}. Falling back to admin queues only...",
                    e
                );
                free_queue(mapper, queue)?;

                self.num_io_queues = 0;
                return Ok(());
            }
        }

        match self.create_sq(1, 2) {
            Ok(_) => {}
            Err(e) => {
                warning!(
                    "(NVME) Unable to create submission queue! Error: {:?}. Falling back to admin queues only...",
                    e
                );
                let mut cmd = NVMeCommand {
                    opcode: NVME_ADMIN_DELETE_CQ,
                    flags: 0,
                    command_id: 0,
                    nsid: 0,
                    rsv0: 0,
                    metadata: 0,
                    prp1: 0,
                    prp2: 0,
                    cdw10: 1,
                    cdw11: 0,
                    cdw12: 0,
                    cdw13: 0,
                    cdw14: 0,
                    cdw15: 0,
                };
                self.submit_admin_cmd(&mut cmd as *mut _, 0 as *mut _, NVME_COMMAND_TIMEOUT_MS)?;
                free_queue(mapper, queue)?;

                self.num_io_queues = 0;
                return Ok(());
            }
        }

        self.num_io_queues = 1;
        return Ok(());
    }
}

struct NVMeNamespace {
    nsid: u32,
    size: u64,
    lba_size: u32,
    lba_shift: u16,
    valid: bool,
    ns_data: *mut NVMeIDNS,
}

#[repr(C, packed)]
struct NVMeIDNS {
    nsze: u64,
    ncap: u64,
    nuse: u64,
    nsfeat: u8,
    nlbaf: u8,
    flbas: u8,
    mc: u8,
    dpc: u8,
    dps: u8,
    nmic: u8,
    rescap: u8,
    fpi: u8,
    dlfeat: u8,
    nawun: u16,
    nawupf: u16,
    nacwu: u16,
    nabsn: u16,
    nabo: u16,
    nabspf: u16,
    noiob: u16,
    nvmcap: [u8; 16],
    npwg: u16,
    npwa: u16,
    npdg: u16,
    npda: u16,
    nows: u16,
    rsvd74: [u8; 18],
    anagrpid: u32,
    rsvd96: [u8; 3],
    nsattr: u8,
    nvmsetid: u16,
    endgid: u16,
    nguid: [u8; 16],
    eui64: [u8; 8],
    lbaf: [Lbaf; 16],
    rsvd192: [u8; 192],
    vs: [u8; 3712],
}

#[repr(C, packed)]
struct Lbaf {
    ms: u16,
    lbads: u8,
    rp: u8,
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
    rsvd100: [u8; 156],
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
    rsvd332: [u8; 180],
    sqes: u8,
    cqes: u8,
    maxcmd: u16,
    nn: u32,
    oncs: u16,
    fuses: u16,
    fna: u8,
    vwc: u8,
    awun: u16,
    awupf: u16,
    nvscc: u8,
    rsvd531: u8,
    acwu: u16,
    rsvd534: u16,
    sgls: u32,
    rsvd540: [u8; 228],
    subnqn: [i8; 256],
    rsvd1024: [u8; 768],
    nvmsr: [u8; 256],
    vs: [u8; 1024],
}

#[derive(Debug)]
pub enum NVMeError {
    NotFound,
    InitFailed,
    Timeout,
    InvalidParam,
    NoMemory,
    DeviceError,
    NotReady,
    CommandFailed,
    NotInitialized,
    OutOfMemory,
    NoQueue,
    RWRegError,
    NamespaceNotFound,
}

pub unsafe fn nvme_init(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> NVMeResult<()> {
    let devs = pci::scan_pci_bus();
    for mut dev in devs {
        if dev.class_code == NVME_PCI_CLASS
            && dev.subclass == NVME_PCI_SUBCLASS
            && dev.prog_if() == NVME_PCI_PROG_IF
        {
            probe_controller(mapper, frame_allocator, &mut dev)?;
        }
    }

    return Ok(());
}

fn map_device_memory(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    phys_addr: u64,
    size: usize,
) -> Option<VirtAddr> {
    if phys_addr < 0x100000000 {
        return Some(VirtAddr::new(phys_addr + unsafe { crate::PMO }));
    }

    let pages_needed = size.div_ceil(4096);
    let virt_base = match alloc_pages(mapper, frame_allocator, pages_needed) {
        Some(v) => v,
        None => {
            error!("(NVME) Unable to allocate {} pages!", pages_needed);
            return None;
        }
    };

    for i in 0..pages_needed {
        match free(mapper, virt_base + (i as u64 * 4096), 4096) {
            Ok(_) => {}
            Err(e) => {
                warning!(
                    "(NVME) Unable to unmap page. Assuming okay... Error: {:?}",
                    e
                );
            }
        }
    }

    for i in 0..pages_needed {
        let virt_page = virt_base.as_u64() + (i as u64 * 4096);
        let phys_page = phys_addr + (i as u64 * 4096);

        match map_page(
            mapper,
            virt_page,
            phys_page,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        ) {
            Ok(_) => {}
            Err(e) => {
                error!("(NVME) Unable to map page for device! Error: {:?}", e);
                return None;
            }
        }
    }

    return Some(virt_base);
}

unsafe fn alloc_aligned_buffer(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    size: usize,
) -> Option<VirtAddr> {
    let pages = (size + Size4KiB::SIZE as usize - 1) / Size4KiB::SIZE as usize;

    let buf = alloc_pages(mapper, frame_allocator, pages)?;

    let phys = buf.as_u64() - crate::PMO;
    if phys & (Size4KiB::SIZE - 1) != 0 {
        free(mapper, buf, pages * Size4KiB::SIZE as usize).expect("(NVME) Unable to unmap buffer!");
        return None;
    }

    return Some(buf);
}

unsafe fn probe_controller(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    dev: &mut PCIDevice,
) -> NVMeResult<()> {
    info!(
        "(NVME) Probing PCI(e) device at: bus: {}, device_id: {}, func: {}",
        dev.bus, dev.device_id, dev.func
    );

    let controller = match alloc(mapper, frame_allocator, size_of::<NVMeController>()) {
        Some(v) => v.as_mut_ptr::<NVMeController>(),
        None => {
            error!(
                "(NVME) Unable to allocate {} bytes for controller!",
                size_of::<NVMeController>()
            );
            return Err(NVMeError::OutOfMemory);
        }
    };

    (*controller).pci_dev = dev as *mut _;

    let page = Page::containing_address(VirtAddr::new(match dev.bar_address(0) {
        Some(a) => a + crate::PMO,
        None => {
            error!("(NVME) Unable to get BAR #0 from PCI(e) device.");
            return Err(NVMeError::InitFailed);
        }
    }));

    match mapper.map_to(
        page,
        match frame_allocator.allocate_frame() {
            Some(f) => f,
            None => {
                error!("(NVME) Unable to allocate a frame for BAR #0 of the PCI(e) device!");
                return Err(NVMeError::InitFailed);
            }
        },
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        frame_allocator,
    ) {
        Ok(f) => f.flush(),
        Err(MapToError::PageAlreadyMapped(_)) => {
            warning!("(NVME) Page for PCI(e) device's BAR #0 is already mapped.");
        }
        Err(e) => {
            error!(
                "(NVME) Unable to map BAR #0 for PCI(e) device! Error: {:?}",
                e
            );
            return Err(NVMeError::InitFailed);
        }
    }

    if !dev.enable_bus_master() {
        error!("(NVME) Unable to enable bus mastering on PCI(e) device!");
        return Err(NVMeError::InitFailed);
    }

    match (*controller).init() {
        Ok(_) => {}
        Err(e) => {
            error!("(NVME) Unable to initialize controller! Error: {:?}", e);
            match free(
                mapper,
                VirtAddr::new(controller as u64),
                size_of::<NVMeController>(),
            ) {
                Ok(_) => {}
                Err(ee) => {
                    error!(
                        "(NVME) Unable to free {} bytes for NVMe controller! Error: {:?}",
                        size_of::<NVMeController>(),
                        ee
                    );
                }
            }
            return Err(e);
        }
    }

    match (*controller).identify(mapper, frame_allocator) {
        Ok(_) => {}
        Err(e) => {
            error!("(NVME) Unable to identify NVMe controller! Error: {:?}", e);
            match free_queue(mapper, &mut (*controller).admin_queue as *mut _) {
                Ok(_) => {}
                Err(ee) => {
                    error!("(NVME) Failed to free admin queue! Error: {:?}", ee);
                }
            }

            match free(
                mapper,
                VirtAddr::new(controller as u64),
                size_of::<NVMeController>(),
            ) {
                Ok(_) => {}
                Err(ee) => {
                    error!(
                        "(NVME) Unable to free {} bytes for NVMe controller! Error: {:?}",
                        size_of::<NVMeController>(),
                        ee
                    );
                }
            }

            return Err(e);
        }
    }

    match (*controller).init_io_queues(mapper, frame_allocator) {
        Ok(_) => {}
        Err(e) => {
            error!("(NVME) Unable to initialize IO queues! Error: {:?}", e);

            match free(
                mapper,
                VirtAddr::new((*controller).ctrl_data as u64),
                size_of::<NVMeIDCtrl>(),
            ) {
                Ok(_) => {}
                Err(ee) => {
                    error!(
                        "(NVME) Unable to free NVMe controller's data! Error: {:?}",
                        ee
                    );
                }
            }

            match free_queue(mapper, &mut (*controller).admin_queue as *mut _) {
                Ok(_) => {}
                Err(e) => {
                    error!("(NVME) Failed to free admin queue! Error: {:?}", e);
                }
            }

            match free(
                mapper,
                VirtAddr::new(controller as u64),
                size_of::<NVMeController>(),
            ) {
                Ok(_) => {}
                Err(ee) => {
                    error!(
                        "(NVME) Unable to free {} bytes for NVMe controller! Error: {:?}",
                        size_of::<NVMeController>(),
                        ee
                    );
                }
            }

            return Err(e);
        }
    }

    match (*controller).init_namespaces(mapper, frame_allocator) {
        Ok(_) => {}
        Err(e) => {
            warning!(
                "(NVME) Unable to initialize NVMe controller's namespaces! Error: {:?}",
                e
            );
        }
    }

    return Ok(());
}

unsafe fn free_queue(mapper: &mut impl Mapper<Size4KiB>, queue: *mut NVMeQueue) -> NVMeResult<()> {
    if queue.is_null() {
        error!("(NVME) Given queue is null!");
        return Err(NVMeError::InvalidParam);
    }

    if !(*queue).sq_cmds.is_null() {
        let free_amount =
            (*queue).q_depth as usize * size_of::<NVMeCommand>() + Size4KiB::SIZE as usize - 1;
        match free(mapper, VirtAddr::new((*queue).sq_cmds as u64), free_amount) {
            Ok(_) => {}
            Err(e) => {
                error!("(NVME) Unable to free sq cmds! Error: {:?}", e);
            }
        }
        (*queue).sq_cmds = 0 as *mut _;
    }

    if !(*queue).cqes.is_null() {
        let free_amount =
            (*queue).q_depth as usize * size_of::<NVMeCompletion>() + Size4KiB::SIZE as usize - 1;
        match free(mapper, VirtAddr::new((*queue).cqes as u64), free_amount) {
            Ok(_) => {}
            Err(e) => {
                error!("(NVME) Unable to free cqes! Error: {:?}", e);
            }
        }
        (*queue).cqes = 0 as *mut _;
    }

    return Ok(());
}

unsafe fn shutdown(mapper: &mut impl Mapper<Size4KiB>, controllers: Vec<NVMeController>) {
    'cl: for mut controller in controllers {
        if controller.enabled {
            controller.write_reg32(
                NVME_REG_CC as u32,
                match controller.read_reg32(NVME_REG_CC as u32) {
                    Some(c) => c & !NVME_CC_EN as u32 | NVME_CC_SHN_NORMAL as u32,
                    None => {
                        error!("(NVME) Couldn't read CC register on NVMe controller!");
                        continue;
                    }
                },
            );

            match controller.wait_ready(false, NVME_SHUTDOWN_TIMEOUT_MS) {
                Ok(_) => {}
                Err(e) => {
                    error!("(NVME) Failed waiting for shutdown! Error: {:?}", e);
                    continue;
                }
            }

            match free_queue(mapper, &mut controller.admin_queue as *mut _) {
                Ok(_) => {}
                Err(e) => {
                    error!("(NVME) Unable to free admin queue! Error: {:?}", e);
                    continue;
                }
            }

            for q in 0..controller.num_io_queues {
                match free_queue(mapper, &mut controller.io_queues[q as usize] as *mut _) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("(NVME) Unable to free IO queue #{}! Error: {:?}", q, e);
                        continue 'cl;
                    }
                }
            }

            match free(
                mapper,
                VirtAddr::new(controller.ctrl_data as u64),
                size_of::<NVMeIDCtrl>(),
            ) {
                Ok(_) => {}
                Err(e) => {
                    error!("(NVME) Unable to free controller data! Error: {:?}", e);
                    continue;
                }
            }

            nftodo!("(NVME) shutdown: Free namespace data");

            match free(
                mapper,
                VirtAddr::new(&controller as *const _ as u64),
                size_of::<NVMeController>(),
            ) {
                Ok(_) => {}
                Err(e) => {
                    error!("(NVME) Unable to free controller! Error: {:?}", e);
                    continue;
                }
            }
        }
    }
}

pub unsafe fn run_basic_test(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    controller: &mut NVMeController,
) -> bool {
    if !controller.enabled {
        error!("(NVME) Unable to test controller, as it is not enabled!");
        return false;
    }

    let mut nsid = 0;
    for i in 1..256 {
        match controller.get_namespace(i) {
            Ok(_) => {
                nsid = i;
            }
            Err(e) => {
                warning!(
                    "(NVME) Unable to get namespace #{}! Error: {:?}. Going to next candidate...",
                    i,
                    e
                );
                continue;
            }
        }
    }

    if nsid == 0 {
        error!("(NVME) No valid namespace candidates.");
        return false;
    }

    let ns = match controller.get_namespace(nsid) {
        Ok(n) => n,
        Err(e) => {
            error!(
                "(NVME) Could not get namespace #{} again! Error: {:?}",
                nsid, e
            );
            return false;
        }
    };

    let mut test_lba = 0x1000;
    if test_lba >= ns.size {
        test_lba /= 2;
    }

    let write_buf = match alloc_aligned_buffer(mapper, frame_allocator, ns.lba_size as usize) {
        Some(v) => v.as_mut_ptr(),
        None => {
            error!("(NVME) Unable to allocate aligned buffer for writing!");
            return false;
        }
    };
    let read_buf = match alloc_aligned_buffer(mapper, frame_allocator, ns.lba_size as usize) {
        Some(v) => v.as_mut_ptr(),
        None => {
            error!("(NVME) Unable to allocate aligned buffer for reading!");
            return false;
        }
    };

    let mut message = *b"lemoncake is cool i guess";
    write_bytes(write_buf, 0, ns.lba_size as usize);
    copy_nonoverlapping(write_buf, &mut message as *mut _, message.len() + 1);
    let lba_sz = ns.lba_size as usize;

    match controller.write_blocks(mapper, frame_allocator, nsid, test_lba, 1, write_buf) {
        Ok(_) => {
            info!("(NVME) Successfully wrote testing block!");
        }
        Err(e) => {
            error!("(NVME) Unable to write testing block! Error: {:?}", e);
            match free(
                mapper,
                VirtAddr::new(write_buf as u64),
                lba_sz + Size4KiB::SIZE as usize - 1 / Size4KiB::SIZE as usize,
            ) {
                Ok(_) => {}
                Err(e) => {
                    error!("(NVME) Unable to free write buffer! Error: {:?}", e);
                    return false;
                }
            }

            match free(
                mapper,
                VirtAddr::new(read_buf as u64),
                lba_sz + Size4KiB::SIZE as usize - 1 / Size4KiB::SIZE as usize,
            ) {
                Ok(_) => {}
                Err(e) => {
                    error!("(NVME) Unable to free write buffer! Error: {:?}", e);
                    return false;
                }
            }
            return false;
        }
    }

    write_bytes(read_buf, 0xFF, lba_sz);

    match controller.read_blocks(mapper, frame_allocator, nsid, test_lba, 1, read_buf) {
        Ok(_) => {}
        Err(e) => {
            error!("(NVME) Unable to read testing block! Error: {:?}", e);
            match free(
                mapper,
                VirtAddr::new(write_buf as u64),
                lba_sz + Size4KiB::SIZE as usize - 1 / Size4KiB::SIZE as usize,
            ) {
                Ok(_) => {}
                Err(e) => {
                    error!("(NVME) Unable to free write buffer! Error: {:?}", e);
                    return false;
                }
            }

            match free(
                mapper,
                VirtAddr::new(read_buf as u64),
                lba_sz + Size4KiB::SIZE as usize - 1 / Size4KiB::SIZE as usize,
            ) {
                Ok(_) => {}
                Err(e) => {
                    error!("(NVME) Unable to free write buffer! Error: {:?}", e);
                    return false;
                }
            }
            return false;
        }
    }

    if compare_bytes(write_buf, read_buf, message.len() + 1) == 0 {
        info!("(NVME) Successfully read from testing block!");
    } else {
        error!("(NVME) Successfully read from testing block, but data read wasn't expected.");
        match free(
            mapper,
            VirtAddr::new(write_buf as u64),
            lba_sz + Size4KiB::SIZE as usize - 1 / Size4KiB::SIZE as usize,
        ) {
            Ok(_) => {}
            Err(e) => {
                error!("(NVME) Unable to free write buffer! Error: {:?}", e);
                return false;
            }
        }

        match free(
            mapper,
            VirtAddr::new(read_buf as u64),
            lba_sz + Size4KiB::SIZE as usize - 1 / Size4KiB::SIZE as usize,
        ) {
            Ok(_) => {}
            Err(e) => {
                error!("(NVME) Unable to free write buffer! Error: {:?}", e);
                return false;
            }
        }
        return false;
    }

    match free(
        mapper,
        VirtAddr::new(write_buf as u64),
        lba_sz + Size4KiB::SIZE as usize - 1 / Size4KiB::SIZE as usize,
    ) {
        Ok(_) => {}
        Err(e) => {
            error!("(NVME) Unable to free write buffer! Error: {:?}", e);
            return false;
        }
    }

    match free(
        mapper,
        VirtAddr::new(read_buf as u64),
        lba_sz + Size4KiB::SIZE as usize - 1 / Size4KiB::SIZE as usize,
    ) {
        Ok(_) => {}
        Err(e) => {
            error!("(NVME) Unable to free write buffer! Error: {:?}", e);
            return false;
        }
    }

    return true;
}
