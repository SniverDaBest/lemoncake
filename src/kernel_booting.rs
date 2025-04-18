#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct KernelHeader {
    /// Should **ALWAYS** be equal to b"kb00t!"
    pub magic: [u8; 6],
    /// Kernel header revision number
    pub revision: u16,
    /// The entry point offset
    pub entry_offset: u64,
}

impl KernelHeader {
    pub fn from_memory(ptr: *const u8) -> (Self, &'static str) {
        let hptr = ptr as *const KernelHeader;
        let header = unsafe { &*hptr };
        let mut info = "";

        if header.magic != *b"kb00t!" {
            panic!("Invalid kernel magic!");
        }

        if header.revision != 1 {
            info = "WARNING: Using kernel with a boot revision other than 1. Things may not work!";
        }

        return (*header, info);
    }

    pub unsafe fn get_entry(
        &self,
        kernel_data: &[u8],
    ) -> (unsafe extern "C" fn(*mut u8) -> !, *const u8, *const u8) {
        let base_ptr = kernel_data.as_ptr();
        let entry_ptr = base_ptr.add(self.entry_offset as usize);
        let addr = *(entry_ptr as *const usize);
        let func = core::mem::transmute::<usize, unsafe extern "C" fn(*mut u8) -> !>(addr);
        return (func, self.entry_offset as *const u8, entry_ptr);
    }
}
