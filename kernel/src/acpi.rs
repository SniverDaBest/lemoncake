use crate::{PMO, info, warning};
use acpi::AcpiTables;
use acpi::{AcpiTable, PhysicalMapping, mcfg::Mcfg, sdt::SdtHeader};
use core::mem::size_of;
use core::ptr::NonNull;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        FrameAllocator, Page, PageSize, PageTableFlags, PhysFrame, Size4KiB, Translate,
        mapper::MapperAllSizes,
    },
};

#[repr(C, packed)]
struct McfgEntry {
    base_address: u64,
    pci_segment_group: u16,
    start_bus: u8,
    end_bus: u8,
    reserved: u32,
}

#[repr(C, packed)]
struct McfgHeader {
    header: SdtHeader,
    reserved: u64,
}

pub unsafe fn init_pcie_from_acpi<M, F>(
    tables: &AcpiTables<Handler>,
    mapper: &mut M,
    frame_allocator: &mut F,
) -> Result<(), &'static str>
where
    M: MapperAllSizes + Translate,
    F: FrameAllocator<Size4KiB>,
{
    let maybe_map = tables.find_table::<Mcfg>();

    let mapping: PhysicalMapping<Handler, Mcfg> = match maybe_map {
        Ok(m) => m,
        Err(_) => return Err("MCFG table not found."),
    };

    let virt_ptr = mapping.virtual_start().as_ptr() as *const u8;
    let total_len = (*mapping.virtual_start().as_ptr()).header().length as usize;

    let hdr_size = size_of::<McfgHeader>();
    if total_len <= hdr_size {
        return Err("MCFG table has no entries");
    }
    let entries_bytes = total_len - hdr_size;
    if !entries_bytes.is_multiple_of(size_of::<McfgEntry>()) {
        return Err("MCFG entries size not multiple of McfgEntry");
    }
    let entry_count = entries_bytes / size_of::<McfgEntry>();
    if entry_count == 0 {
        return Err("MCFG: zero entries");
    }

    let entries_ptr = virt_ptr.add(hdr_size) as *const McfgEntry;

    let mut chosen: Option<&McfgEntry> = None;
    for i in 0..entry_count {
        let e = &*entries_ptr.add(i);
        if e.pci_segment_group == 0 {
            chosen = Some(e);
            break;
        }
    }
    let entry = match chosen {
        Some(e) => e,
        None => return Err("No MCFG entry for segment 0"),
    };

    let bus_count = (entry.end_bus as usize).saturating_sub(entry.start_bus as usize) + 1;
    let ecam_size = bus_count.checked_shl(20).ok_or("ECAM size overflow")?;

    let phys_base = PhysAddr::new(entry.base_address);

    let virt_base = map_ecam_region(phys_base, ecam_size, mapper, frame_allocator)
        .ok_or("Failed to map ECAM region")?;

    crate::pci::set_ecam_base(virt_base.as_u64() as usize);

    let ba = entry.base_address;

    info!(
        "(ACPI) Mapped ECAM at {:#x} ({:#x}). Bus range: {}..{} ({} bytes)",
        ba,
        virt_base.as_u64(),
        entry.start_bus,
        entry.end_bus,
        ecam_size
    );

    return Ok(());
}

pub unsafe fn map_ecam_region<M, F>(
    phys_base: PhysAddr,
    size_bytes: usize,
    mapper: &mut M,
    frame_allocator: &mut F,
) -> Option<VirtAddr>
where
    M: MapperAllSizes + Translate,
    F: FrameAllocator<Size4KiB>,
{
    let start_frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(phys_base);
    let end_phys = phys_base + (size_bytes as u64) - 1u64;
    let end_frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(end_phys);

    let start_addr = start_frame.start_address().as_u64();
    let end_addr = end_frame.start_address().as_u64();
    let frame_count = ((end_addr - start_addr) / Size4KiB::SIZE) + 1;

    let virt_base = VirtAddr::new(phys_base.as_u64() + crate::PMO);

    if let Some(mapped_phys) = mapper.translate_addr(virt_base) {
        if mapped_phys.as_u64() == start_frame.start_address().as_u64() {
            info!(
                "(ACPI) ECAM virt range already mapped at virt {:#x} ({:#x}) with size of {} bytes.",
                virt_base.as_u64(),
                mapped_phys.as_u64(),
                size_bytes
            );
            return Some(virt_base);
        } else {
            warning!(
                "(ACPI) ECAM virt base {:#x} is mapped to different phys {:#x}. Will not remap.",
                virt_base.as_u64(),
                mapped_phys.as_u64()
            );
            return None;
        }
    }

    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    let mut current_frame = start_frame;
    let mut current_page = Page::containing_address(virt_base);

    for i in 0..frame_count {
        match mapper.map_to(current_page, current_frame, flags, frame_allocator) {
            Ok(flush) => {
                flush.flush();
            }
            Err(e) => {
                warning!(
                    "(ACPI) Unable to map frame {} at {:#x} ({:#x}). Error: {:?}",
                    i,
                    current_page.start_address().as_u64(),
                    current_frame.start_address().as_u64(),
                    e
                );
                return None;
            }
        }

        current_page =
            Page::from_start_address(current_page.start_address() + Size4KiB::SIZE).ok()?;
        current_frame =
            PhysFrame::from_start_address(current_frame.start_address() + Size4KiB::SIZE).ok()?;
    }

    return Some(virt_base);
}

#[derive(Clone, Debug)]
pub struct Handler;

impl ::acpi::AcpiHandler for Handler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        PhysicalMapping::new(
            physical_address,
            NonNull::new((physical_address + PMO as usize) as *mut T)
                .expect("Couldn't create a NonNull!"),
            size,
            size,
            self.clone(),
        )
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
}
