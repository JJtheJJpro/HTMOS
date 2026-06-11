use core::{arch::asm, mem::size_of};

const PAGE_TABLE_BASE: usize = 0x0100_0000;

pub fn init() {
    create_mappings();

    enable_paging();
}

fn create_mappings() {
    // Lay out tables consecutively from PAGE_TABLE_BASE:
    // L4 at base+0, L3 at base+0x1000, L2[0..10] at base+0x2000..0xC000
    let l4 = PAGE_TABLE_BASE as *mut PageTable;
    let l3 = (PAGE_TABLE_BASE + 0x1000) as *mut PageTable;
    let l2_base = (PAGE_TABLE_BASE + 0x2000) as *mut PageTable;

    // Zero all tables first (they may contain garbage)
    unsafe {
        core::ptr::write_bytes(l4, 0, 1);
        core::ptr::write_bytes(l3, 0, 1);
        core::ptr::write_bytes(l2_base, 0, 10);
    }

    let common_flags: u64 = 0b11; // PRESENT | WRITABLE

    // PML4[0] -> L3
    unsafe {
        (*l4).entries[0] = l3 as u64 | common_flags;
    }

    // L3[0..10] -> L2[0..10], each covering 1GB
    for i in 0..10usize {
        let l2 = unsafe { l2_base.add(i) };
        unsafe {
            (*l3).entries[i] = l2 as u64 | common_flags;
        }

        let offset = i as u64 * 1024 * 1024 * 1024;
        for j in 0..512usize {
            unsafe {
                (*l2).entries[j] = (offset + j as u64 * 2 * 1024 * 1024) | common_flags | (1 << 7);
                // PS bit: 2MB page
            }
        }
    }
}

fn enable_paging() {
    // load level 4 table pointer into cr3 register
    let l4 = PAGE_TABLE_BASE as *mut PageTable;
    unsafe { asm!("mov cr3, {0}", in(reg) l4) };
    unsafe { asm!("mov eax, cr4", "or eax, 1<<5", "mov cr4, eax", out("eax") _) };
    unsafe {
        asm!("mov ecx, 0xC0000080", "rdmsr", "or eax, 1<<8", "wrmsr", out("eax") _, out("ecx") _)
    };
    unsafe { asm!("mov eax, cr0", "or eax, 1<<31", "mov cr0, eax", out("eax") _) };
}

#[derive(Clone, Copy)]
#[repr(align(4096))]
struct PageTable {
    pub entries: [u64; 512],
}

pub static LONG_MODE_GDT: GdtLongMode = GdtLongMode::new();

#[repr(C)]
pub struct GdtLongMode {
    zero: u64,
    code: u64,
    data: u64,
}

impl GdtLongMode {
    const fn new() -> Self {
        let common_flags = {
            (1 << 44) // user segment
            | (1 << 47) // present
            | (1 << 41) // writable
            | (1 << 40) // accessed (to avoid changes by the CPU)
        };
        Self {
            zero: 0,
            code: common_flags | (1 << 43) | (1 << 53), // executable and long mode
            data: common_flags,
        }
    }

    pub fn load(&'static self) {
        let pointer = GdtPointer {
            base: self,
            limit: (3 * size_of::<u64>() - 1) as u16,
        };

        unsafe {
            asm!("lgdt [{}]", in(reg) &pointer, options(readonly, nostack, preserves_flags));
        }
    }
}

#[repr(C, packed(2))]
pub struct GdtPointer {
    /// Size of the DT.
    pub limit: u16,
    /// Pointer to the memory region containing the DT.
    pub base: *const GdtLongMode,
}

unsafe impl Send for GdtPointer {}
unsafe impl Sync for GdtPointer {}
