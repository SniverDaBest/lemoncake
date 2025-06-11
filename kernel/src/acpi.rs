use crate::{PMO, nftodo};
use acpi::PhysicalMapping;
use core::ptr::NonNull;

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

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {
        nftodo!("(ACPI) Implement unmapping physical ACPI region");
    }
}
