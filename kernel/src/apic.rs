//! Large portions of this code are NOT MINE! They are from the Ruddle/Fomos repo on GitHub.

use crate::PMO;
use conquer_once::spin::OnceCell;
use core::intrinsics::{volatile_load, volatile_store};
use raw_cpuid::{CpuId, CpuIdResult};
use x86_64::{
    PhysAddr, VirtAddr,
    registers::model_specific::Msr,
};

pub fn cpuid() -> Option<CpuId> {
    Some(CpuId::with_cpuid_fn(|a, c| {
        let result = unsafe { core::arch::x86_64::__cpuid_count(a, c) };
        CpuIdResult {
            eax: result.eax,
            ebx: result.ebx,
            ecx: result.ecx,
            edx: result.edx,
        }
    }))
}

pub static LAPIC: OnceCell<LocalApic> = OnceCell::uninit();

pub struct LocalApic {
    pub virt_address: VirtAddr,
}

impl LocalApic {
    pub unsafe fn init(local_apic_address: PhysAddr) -> &'static Self {
        disable_pic();

        let virtaddr: VirtAddr = VirtAddr::new(local_apic_address.as_u64() + PMO);
        let ret = LAPIC.get_or_init(|| Self {
            virt_address: virtaddr,
        });

        let mut msr = Msr::new(0x1B);
        let r = msr.read();
        msr.write(r | (1 << 11));

        ret.write(0xF0, ret.read(0xF0) | 0x1FF);
        return ret;
    }

    unsafe fn read(&self, reg: u32) -> u32 {
        volatile_load((self.virt_address.as_u64() + reg as u64) as *const u32)
    }

    unsafe fn write(&self, reg: u32, value: u32) {
        volatile_store((self.virt_address.as_u64() + reg as u64) as *mut u32, value);
    }

    pub fn id(&self) -> u32 {
        unsafe { self.read(0x20) }
    }

    pub fn version(&self) -> u32 {
        unsafe { self.read(0x30) }
    }

    pub fn icr(&self) -> u64 {
        unsafe { (self.read(0x310) as u64) << 32 | self.read(0x300) as u64 }
    }

    pub fn set_icr(&self, value: u64) {
        unsafe {
            const PENDING: u32 = 1 << 12;
            while self.read(0x300) & PENDING == PENDING {
                core::hint::spin_loop();
            }
            self.write(0x310, (value >> 32) as u32);
            self.write(0x300, value as u32);
            while self.read(0x300) & PENDING == PENDING {
                core::hint::spin_loop();
            }
        }
    }

    pub fn ipi(&self, apic_id: usize) {
        let mut icr = 0x4040;

        icr |= (apic_id as u64) << 56;

        self.set_icr(icr);
    }

    pub fn ipi_nmi(&self, apic_id: u32) {
        let shift = { 56 };
        self.set_icr((u64::from(apic_id) << shift) | (1 << 14) | (0b100 << 8));
    }

    pub unsafe fn eoi(&self) {
        self.write(0xB0, 0);
    }

    pub unsafe fn esr(&self) -> u32 {
        self.write(0x280, 0);
        self.read(0x280)
    }
    pub unsafe fn lvt_timer(&self) -> u32 {
        self.read(0x320)
    }
    pub unsafe fn set_lvt_timer(&self, value: u32) {
        self.write(0x320, value);
    }
    pub unsafe fn init_count(&self) -> u32 {
        self.read(0x380)
    }
    pub unsafe fn set_init_count(&self, initial_count: u32) {
        self.write(0x380, initial_count);
    }
    pub unsafe fn cur_count(&self) -> u32 {
        self.read(0x390)
    }
    pub unsafe fn div_conf(&self) -> u32 {
        self.read(0x3E0)
    }
    pub unsafe fn set_div_conf(&self, div_conf: u32) {
        self.write(0x3E0, div_conf);
    }
    pub unsafe fn lvt_error(&self) -> u32 {
        self.read(0x370)
    }
    pub unsafe fn set_lvt_error(&self, lvt_error: u32) {
        self.write(0x370, lvt_error);
    }
    #[allow(unused)]
    unsafe fn setup_error_int(&self) {
        let vector = 49u32;
        self.set_lvt_error(vector);
    }
}
pub unsafe fn disable_pic() {
    use x86_64::instructions::port::Port;
    let mut wait_port: Port<u8> = Port::new(0x80);
    let mut wait = || {
        wait_port.write(0);
    };
    let mut p0c: Port<u8> = Port::new(0x20);
    let mut p0d: Port<u8> = Port::new(0x21);
    let mut p1c: Port<u8> = Port::new(0xA0);
    let mut p1d: Port<u8> = Port::new(0xA1);
    p0c.write(0x11);
    wait();
    p1c.write(0x11);
    wait();

    p0d.write(0xf0);
    wait();
    p1d.write(0xf8);
    wait();
    p0d.write(0x4);
    wait();
    p1d.write(0x2);
    wait();
    p0d.write(0x1);
    wait();
    p1d.write(0x1);
    wait();
    p0d.write(0xff);
    wait();
    p1d.write(0xff);
    wait();
}

pub static IO_APIC_0: OnceCell<IoApic> = OnceCell::uninit();

#[allow(unused)]
pub struct IoApic {
    virt_address: VirtAddr,
    global_system_int: u32,
    id: u8,
}

pub const IOAPICID: u32 = 0;
pub const IOAPICVER: u32 = 1;
impl IoApic {
    pub fn init(info: &acpi::platform::interrupt::IoApic) -> &Self {
        let this = IO_APIC_0.get_or_init(move || Self {
            id: info.id,
            virt_address: unsafe {
                VirtAddr::new(PhysAddr::new(info.address as u64).as_u64() + PMO)
            },
            global_system_int: info.global_system_interrupt_base,
        });
        this
    }
    pub unsafe fn set_sel(&self, reg: u32) {
        volatile_store(self.virt_address.as_u64() as *mut u32, reg);
    }
    pub fn read(&self, reg: u32) -> u32 {
        unsafe {
            self.set_sel(reg);
            let sec: VirtAddr = self.virt_address + 0x10_u64;
            volatile_load(sec.as_u64() as *const u32)
        }
    }
    pub fn write(&self, reg: u32, value: u32) {
        unsafe {
            self.set_sel(reg);
            let sec: VirtAddr = self.virt_address + 0x10_u64;
            volatile_store(sec.as_u64() as *mut u32, value);
        }
    }

    pub fn read_redtlb(&self, index: u32) -> u64 {
        let low = self.read(0x10 + 2 * index) as u64;
        let high = self.read(0x10 + 2 * index + 1) as u64;
        (high << 32) + low
    }
    pub fn write_redtlb(&self, index: u32, redtlb: u64) {
        self.write(0x10 + 2 * index, (redtlb & 0xffff) as u32);
        self.write(0x10 + 2 * index + 1, (redtlb >> 32) as u32);
    }
}
#[derive(Clone, Debug)]
pub struct RedTbl {
    pub vector: u8,
    pub delivery_mode: u8,
    pub destination_mode: bool,
    pub delivery_status: bool,
    pub pin_polarity: bool,
    pub remote_irr: bool,
    pub trigger_mode: bool,
    pub mask: bool,
    pub destination: u8,
}

impl RedTbl {
    #[allow(unused_assignments)]
    pub fn new(n: u64) -> Self {
        let mut c = n;
        let vector = (c & 0xff) as u8;
        c >>= 8;
        let delivery_mode = (c & 0b11) as u8;
        c >>= 2;
        let destination_mode = c & 0b1 != 0;
        c >>= 1;
        let delivery_status = c & 0b1 != 0;
        c >>= 1;
        let pin_polarity = c & 0b1 != 0;
        c >>= 1;
        let remote_irr = c & 0b1 != 0;
        c >>= 1;
        let trigger_mode = c & 0b1 != 0;
        c >>= 1;
        let mask = c & 0b1 != 0;
        c >>= 1;
        let destination = (n >> 56) as u8;
        Self {
            vector,
            delivery_mode,
            destination_mode,
            delivery_status,
            pin_polarity,
            remote_irr,
            trigger_mode,
            mask,
            destination,
        }
    }
    pub fn store(&self) -> u64 {
        let &Self {
            vector,
            delivery_mode,
            destination_mode,
            delivery_status,
            pin_polarity,
            remote_irr,
            trigger_mode,
            mask,
            destination,
        } = self;

        let mut r = 0_u64;
        r += (destination as u64) << 56;
        r += vector as u64;
        r += (delivery_mode as u64) << 8;
        r += if destination_mode { 1 } else { 0 } << 10;
        r += if delivery_status { 1 } else { 0 } << 11;
        r += if pin_polarity { 1 } else { 0 } << 12;
        r += if remote_irr { 1 } else { 0 } << 13;
        r += if trigger_mode { 1 } else { 0 } << 14;
        r += if mask { 1 } else { 0 } << 15;
        r
    }
}
