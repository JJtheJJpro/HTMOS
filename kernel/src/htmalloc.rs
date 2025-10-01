//! **HyperText Markup Allocation System**

extern crate alloc;

use crate::boot_info::boot_info;
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::Cell,
    ptr::null_mut,
};

const ARENA_SIZE: usize = 128 * 1024;
const MAX_SUPPORTED_ALIGN: usize = 4096;

pub struct HTMAlloc {
    mmap: Cell<u64>,
    mmap_len: Cell<u64>,
    desc_size: Cell<u64>,
}
impl HTMAlloc {
    pub const fn ginit() -> Self {
        Self {
            mmap: Cell::new(0),
            mmap_len: Cell::new(0),
            desc_size: Cell::new(0),
        }
    }
    pub fn update(&self) {
        let bi = boot_info();
        self.mmap.set(bi.memory_map_addr);
        self.mmap_len.set(bi.memory_map_len);
        self.desc_size.set(bi.memory_desc_size);
    }
}

unsafe impl Sync for HTMAlloc {}
unsafe impl GlobalAlloc for HTMAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        null_mut()
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        null_mut()
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {}
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        null_mut()
    }
}
