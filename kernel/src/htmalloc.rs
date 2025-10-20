//! **HyperText Markup Allocation System**

extern crate alloc;

use alloc::vec::Vec;
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::{Cell, UnsafeCell},
};

//const ARENA_SIZE: usize = 128 * 1024;
//const MAX_SUPPORTED_ALIGN: usize = 4096;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MemoryPattern {
    /// No overlapping
    Separate,
    /// Size changes, start point remains to original section
    StartBranch,
    /// Size changes, start point changes to given section
    EndBranch,
    /// Original section overlaps entire given section
    NoChange,
    /// Given section overlaps entire original section
    Overwrite,
}

const fn end(start: usize, size: usize) -> usize {
    start + size
}
const fn endt(tuple: (usize, usize)) -> usize {
    tuple.0 + tuple.1
}

/// Start branch means first (original) set starts first.
const fn cmp_mem_sct(
    original_start: usize,
    original_size: usize,
    given_start: usize,
    given_size: usize,
) -> MemoryPattern {
    if original_start == given_start && original_size == given_size {
        return MemoryPattern::NoChange;
    }

    let original_end = end(original_start, original_size);
    let given_end = end(given_start, given_size);

    if original_end < given_start || given_end < original_start {
        MemoryPattern::Separate
    } else if given_start == original_end {
        MemoryPattern::StartBranch
    } else if original_start == given_end {
        MemoryPattern::EndBranch
    } else if given_start >= original_start {
        if given_end > original_end {
            if original_start == given_start {
                MemoryPattern::Overwrite
            } else {
                MemoryPattern::StartBranch
            }
        } else {
            MemoryPattern::NoChange
        }
    } else if given_end <= original_end {
        if original_end == given_end {
            MemoryPattern::Overwrite
        } else {
            MemoryPattern::EndBranch
        }
    } else {
        MemoryPattern::Overwrite
    }
}

pub struct HTMAlloc {
    mmap: Cell<([(usize, usize); 256], usize)>,
    taken: UnsafeCell<Vec<(usize, usize)>>,
}
impl HTMAlloc {
    pub const fn ginit() -> Self {
        Self {
            mmap: Cell::new(([(0, 0); 256], 0)),
            taken: UnsafeCell::new(Vec::new()), // I am so glad this won't allocate.
        }
    }
    /// This will perform real-time action after the boot information is handled correctly.
    ///
    /// This will go through each memory chunk, parse all config tables given, and mark free as much as possible.
    pub fn update(&self, mmap: ([(usize, usize); 256], usize)) {
        self.mmap.set(mmap);

        // That should be it for reserved memory.
        // This will be an initialization of the 'taken' member of the HTMAlloc struct...scary stuff.
        {
            let (mmap, sz) = self.mmap.get();
            for i in 0..sz {
                if mmap[i].1 >= size_of::<(usize, usize)>() * 1000 {
                    let ptr = mmap[i].0 as *mut (usize, usize);
                    unsafe {
                        ptr.write((mmap[i].0, size_of::<(usize, usize)>() * 1000));
                    }

                    self.set_taken(mmap[i].0, 1, 1000);

                    break;
                }
            }
        }

        /*
        let mut cfg_ptr = unsafe { &mut *(bi.more_info as *mut SystemTable) }.configuration_table;
        let cfg_count = unsafe { &mut *(bi.more_info as *mut SystemTable) }.number_of_table_entries;
        for i in 0..cfg_count {
            let cfg = unsafe { &*cfg_ptr };
            println!(
                "{}  0x{:16X}",
                str::from_utf8(&crate::cfg_tbl::guid_utf8_upper(cfg.vendor_guid)).unwrap(),
                cfg.vendor_table as usize
            );

            if let Ok(v) = FirmwareTable::parse(cfg.vendor_guid, cfg.vendor_table) {
                match v {
                    FirmwareTable::LZMACustomDecompress(lzma) => {
                        println!(
                            "{}",
                            str::from_utf8(&crate::cfg_tbl::guid_utf8_upper(lzma.guid)).unwrap()
                        );
                        println!("0x{:016X}", lzma.compressed_data.len());
                    }
                }
            }

            cfg_ptr = unsafe { (cfg_ptr as *const u8).add(size_of::<ConfigurationTable>()) }
                as *mut ConfigurationTable;
        }
        */
    }

    fn get_taken_mut(&self) -> &mut Vec<(usize, usize)> {
        unsafe { &mut *self.taken.get() }
    }

    fn get_taken(&self) -> &Vec<(usize, usize)> {
        unsafe { &*self.taken.get() }
    }

    fn set_taken(&self, ptr: usize, length: usize, capacity: usize) {
        unsafe {
            self.taken
                .get()
                .write(Vec::from_raw_parts(ptr as *mut _, length, capacity));
        }
    }

    fn valid_memory_check(&self, ptr: usize, size: usize) -> bool {
        let (mmap, sz) = self.mmap.get();
        for i in 0..sz {
            match cmp_mem_sct(mmap[i].0, mmap[i].1, ptr, size) {
                MemoryPattern::StartBranch => return false,
                MemoryPattern::EndBranch => return false,
                MemoryPattern::Overwrite => return false,
                MemoryPattern::NoChange => return true,
                MemoryPattern::Separate => {}
            }
        }
        false // All were seperate in this case
    }

    /// Returns a pointer to the next valid available section of memory, given that the returned pointer must have an equal or higher value than the given pointer and the given size fits.
    ///
    /// If the given successfully checked pointer is in the middle of a memory section, the return value is the given pointer.
    fn next_valid_memory_ptr(&self, from_ptr: usize, with_size: usize) -> usize {
        if with_size == 0 {
            return 0;
        }
        let (mmap, sz) = self.mmap.get();
        for i in 0..sz {
            if from_ptr >= mmap[i].0 && with_size <= mmap[i].1 {
                match cmp_mem_sct(mmap[i].0, mmap[i].1, from_ptr, with_size) {
                    MemoryPattern::NoChange => return from_ptr,
                    MemoryPattern::Separate => return mmap[i].0,
                    _ => {}
                }
            }
        }
        0
    }

    /// Returns the pointer to the next available section of memory not taken, given the size requirement.
    ///
    /// Returns 0 if size == 0.
    fn next_available_slot(&self, size: usize) -> usize {
        let v = self.get_taken();
        if size == 0 || v.len() == 0 {
            crate::println!("what");
            return 0;
        }
        for i in 0..v.len() {
            let pstart = endt(v[i]);
            if i + 1 < v.len() {
                if pstart + size <= v[i + 1].0 && self.valid_memory_check(pstart, size) {
                    return pstart;
                }
            } else {
                if self.valid_memory_check(pstart, size) {
                    return pstart;
                } else {
                    return self.next_valid_memory_ptr(pstart, size);
                }
            }
        }
        unreachable!(
            "next_available_slot: illegal length mathematics (vec length: {})",
            v.len()
        );
    }
}

unsafe impl Sync for HTMAlloc {}
unsafe impl GlobalAlloc for HTMAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.pad_to_align().size();
        //crate::println!("alloc call size: {size}");
        let ptr = self.next_available_slot(size);
        self.get_taken_mut().push((ptr, size));
        ptr as *mut u8
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let size = layout.pad_to_align().size();
        let ptr = unsafe { self.alloc(layout) };
        unsafe {
            ptr.write_bytes(0, size);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (start, size) = (ptr as usize, layout.pad_to_align().size());
        if self.get_taken().contains(&(start, size)) {
            if !self.get_taken().is_sorted_by(|(v1, _), (v2, _)| v1 <= v2) {
                self.get_taken_mut()
                    .sort_unstable_by(|(s1, _), (s2, _)| s1.cmp(s2));
            }
            self.get_taken_mut().remove(
                self.get_taken()
                    .binary_search_by(|(v, _)| v.cmp(&start))
                    .unwrap(),
            );
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old_size = layout.pad_to_align().size();
        let new_size = Layout::from_size_align(new_size, layout.align())
            .unwrap()
            .pad_to_align()
            .size();

        //crate::println!("from {old_size} to {new_size}");
        if old_size > new_size {
            return ptr;
        }
        if unsafe { &*self.taken.get() }.as_ptr() as usize == ptr as usize {
            todo!("The 'taken' member of the kernel allocation needs more room.");
            // The 'taken' member called.  This will be some sort of "fakeout" or something.
            //let taken_mut = unsafe { &mut *self.taken.get() };
            //taken_mut[0] = (ptr as usize, new_size);
            //ptr
        } else {
            unsafe {
                if self.get_taken().last().unwrap().0 == ptr as usize {
                    *self.get_taken_mut().last_mut().unwrap() = (ptr as usize, new_size);
                    ptr
                } else {
                    let nptr = self
                        .alloc_zeroed(Layout::from_size_align(new_size, layout.align()).unwrap());
                    core::ptr::copy_nonoverlapping(ptr as *const _, nptr, old_size);
                    self.dealloc(ptr, layout);
                    nptr
                }
            }
        }
    }
}
