use crate::{error, info, memory, pci::PCIDevice, warning};
use x86_64::{
    PhysAddr, VirtAddr,
    instructions::port::Port,
    structures::paging::{Mapper, Page, PageTableFlags, PhysFrame, Size4KiB, mapper::MapToError},
};

pub const PCI_VENDOR_ID: u32 = 0x10EC;
pub const PCI_DEVICE_ID: u32 = 0x8139;
static mut RX_BUF: [u8; 8202] = [0u8; 8202];

#[derive(Debug)]
pub enum PacketError {
    TXDescBusy,
    TooLong, // YOUR        TOO LONG
    BadPacket,
    BufferEmpty,
}

pub struct Rtl8139 {
    mac: [u8; 6],
    mar: [u8; 8],
    tsd: [u32; 4],
    tsad: [u32; 4],
    rbstart: u32,
    cmd: u8,
    imr: u16,
    isr: u16,
    tx_current: u8,
    rx_offset: u16,
    ioaddr: u16,
    irq: u8,
}

impl Rtl8139 {
    pub unsafe fn init(pci_dev: PCIDevice, mapper: &mut impl Mapper<Size4KiB>) -> Option<Self> {
        if !pci_dev.enable_bus_master() {
            error!("(RTL8139) Unable to enable bus mastering!");
            return None;
        }

        let frame = PhysFrame::containing_address(PhysAddr::new(pci_dev.bar_address(0)?));
        match mapper.map_to(
            Page::containing_address(VirtAddr::new(pci_dev.bar_address(0)? + crate::PMO)),
            frame,
            PageTableFlags::NO_CACHE | PageTableFlags::PRESENT,
            &mut memory::EmptyFrameAllocator,
        ) {
            Ok(f) => f.flush(),
            Err(e) => {
                error!(
                    "(RTL8139) Unable to map page for PCI device! Error: {:?}.",
                    e
                );
                return None;
            }
        }

        let ioaddr = pci_dev.read_bar(0)?.0 as u16 & !0x3;

        Port::new(ioaddr + 0x52).write(0u8);

        let mut rst_port = Port::new(ioaddr + 0x37);
        rst_port.write(0x10u8);
        while rst_port.read() != 0 { /* Wait for reset */ }

        #[allow(static_mut_refs)]
        Port::new(ioaddr + 0x30).write(RX_BUF.as_mut_ptr() as u32 + crate::PMO as u32);

        let irq = pci_dev.read_config_u8(0x3C);
        info!("(RTL8139) Using IRQ {}", irq);

        Port::new(ioaddr + 0x3c).write(0x0005u16);

        Port::new(ioaddr + 0x44).write(0xfu32 | (1 << 7));

        Port::new(ioaddr + 0x37).write(0x0Cu8);

        let mut mac = [0u8; 6];
        for i in 0..6 {
            mac[i] = Port::<u8>::new(ioaddr + i as u16).read();
        }

        return Some(Self {
            mac,
            mar: [0; 8],
            tsd: [0; 4],
            tsad: [0; 4],
            rbstart: 0,
            cmd: 0,
            imr: 0,
            isr: 0,
            tx_current: 0,
            rx_offset: 0,
            ioaddr,
            irq
        });
    }

    pub unsafe fn transmit_packet(
        &mut self,
        ioaddr: u16,
        packet: &[u8],
    ) -> Result<(), PacketError> {
        if packet.len() > 1792 {
            return Err(PacketError::TooLong);
        }

        let desc = self.tx_current as usize;

        let tsd_offset = 0x10 + (desc * 4) as u16;
        let status: u32 = Port::new(ioaddr + tsd_offset).read();

        if (status & (1 << 13)) == 0 {
            return Err(PacketError::TXDescBusy);
        }

        let tsad_offset = 0x20 + (desc * 4) as u16;
        let packet_phys_addr = packet.as_ptr() as u64 + crate::PMO;
        Port::new(ioaddr + tsad_offset).write(packet_phys_addr as u32);

        Port::new(ioaddr + tsd_offset).write(packet.len() as u32);

        self.tx_current = (self.tx_current + 1) % 4;

        return Ok(());
    }

    unsafe fn update_read_pointer(&self, ioaddr: u16) {
        Port::new(ioaddr + 0x38).write(if self.rx_offset == 0 {
            8192 - 0x10
        } else {
            self.rx_offset - 0x10
        });
    }
    
    pub unsafe fn handle_interrupt(&mut self) {
        let isr: u16 = Port::new(self.ioaddr + 0x3E).read();
        
        if (isr & 0x01) != 0 {
            self.handle_receive();
        }
        
        if (isr & 0x04) != 0 {
            // TOK - Transmit OK (one of the TX descriptors completed)
            // You can track transmission completion here
        }
        
        if (isr & 0x02) != 0 {
            error!("(RTL8139) Receive error!");
        }
        
        Port::new(self.ioaddr + 0x3E).write(isr);
    }
    
    unsafe fn handle_receive(&mut self) {
        while let Ok(packet) = self.receive_packet() {
            // Do something with the packet
            info!("(RTL8139) Received packet of {} bytes", packet.len());
            // TODO: Pass to network stack
        }
    }
    
    unsafe fn receive_packet(&mut self) -> Result<&'static [u8], PacketError> {
        let cmd: u8 = Port::new(self.ioaddr + 0x37).read();
        if (cmd & 0x01) != 0 {
            return Err(PacketError::BufferEmpty);
        }

        let rx_buffer = &raw const RX_BUF as *const [u8; 8202] as *const u8;
        let rx_buffer_base = rx_buffer as usize;

        let header_ptr = (rx_buffer_base + self.rx_offset as usize) as *const u32;
        let header = core::ptr::read_unaligned(header_ptr);

        let status = (header & 0xFFFF) as u16;
        let length = ((header >> 16) & 0xFFFF) as u16;

        if (status & 0x01) == 0 {
            self.rx_offset = (self.rx_offset + length + 4 + 3) & !3;
            self.update_read_pointer(self.ioaddr);
            return Err(PacketError::BadPacket);
        }

        let packet_start = rx_buffer_base + self.rx_offset as usize + 4;
        let packet_length = (length - 4) as usize;
        let packet = core::slice::from_raw_parts(packet_start as *const u8, packet_length);

        self.rx_offset = (self.rx_offset + length + 4 + 3) & !3;

        if self.rx_offset >= 8192 {
            self.rx_offset = 0;
        }

        self.update_read_pointer(self.ioaddr);

        return Ok(packet);
    }
}
