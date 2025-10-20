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

/// The struct in place for global allocations for HTMOS.
pub struct HTMAlloc {
    mmap: Cell<([(usize, usize); 256], usize)>,
    /// This holds any taken memory in the mmap.
    ///
    /// The first element is always an "Apocolyptic Expression" - an allocated chunk for this Vec itself.
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
                if mmap[i].1 >= size_of::<(usize, usize)>() * 4096 {
                    let ptr = mmap[i].0 as *mut (usize, usize);
                    unsafe {
                        ptr.write((mmap[i].0, size_of::<(usize, usize)>() * 4096));
                    }

                    self.set_taken(mmap[i].0, 1, 4096);

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
    /// Returns 0 if size == 0 or the update function hasn't been called yet.
    fn next_available_slot(&self, size: usize) -> usize {
        let v = self.get_taken();
        if size == 0 || v.len() == 0 {
            return 0;
        } else if v.len() > 2 {
            self.taken_organize();
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

    /// Returns an updated value to fit inside the fixed memory map.  Returns (0, 0) if no overlaps are present.
    ///
    /// **NOTE**: If the given section spreads over multiple memory map sections, it will go through with the first instance it finds.
    fn reduce_size_fit_next_available_slot(&self, start: usize, size: usize) -> (usize, usize) {
        let (mmap, sz) = self.mmap.get();
        for i in 0..sz {
            match cmp_mem_sct(mmap[i].0, mmap[i].1, start, size) {
                MemoryPattern::StartBranch => return (start, endt(mmap[i]) - start),
                MemoryPattern::EndBranch => return (mmap[i].0, end(start, size) - mmap[i].0),
                MemoryPattern::Overwrite => return (mmap[i].0, mmap[i].1),
                MemoryPattern::NoChange => return (start, size),
                MemoryPattern::Separate => {}
            }
        }
        (0, 0)
    }

    /// Fixes the order in 'taken'.  Returns true if changes happened.
    fn taken_organize(&self) -> bool {
        let v = self.get_taken_mut();
        if !v[1..].is_sorted_by(|(v0, _), (v1, _)| v0 < v1) {
            v[1..].sort_unstable_by(|(v0, _), (v1, _)| v0.cmp(v1));
            true
        } else {
            false
        }
    }

    /// Returns the index of the given tuple (start and size must match exactly).  Returns None if not found.
    fn taken_match(&self, start: usize, size: usize) -> Option<usize> {
        self.taken_organize();
        let v = self.get_taken();
        if let Ok(i) = v[1..].binary_search_by(|(v, _)| v.cmp(&start)) {
            if v[i].1 == size { Some(i) } else { None }
        } else {
            None
        }
    }

    /// Returns the index of the given start ptr.  Returns None if not found.
    fn taken_match_start(&self, start: usize) -> Option<usize> {
        self.taken_organize();
        let v = self.get_taken();
        if let Ok(i) = v[1..].binary_search_by(|(v, _)| v.cmp(&start)) {
            Some(i)
        } else {
            None
        }
    }

    /// Returns the number of additional bytes available for reallocation.
    fn taken_more_size_available(&self, mstart: usize, msize: usize) -> usize {
        self.taken_organize();
        if let Some(i) = self.taken_match(mstart, msize) {
            let v = self.get_taken();
            let pstart = end(mstart, msize);
            let psz = v[i + 1].0 - pstart;
            if psz > 0 {
                let (good_start, good_size) = self.reduce_size_fit_next_available_slot(pstart, psz);
                if pstart == good_start { good_size } else { 0 }
            } else {
                0
            }
        } else {
            0
        }
    }
}

unsafe impl Sync for HTMAlloc {}
unsafe impl GlobalAlloc for HTMAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.pad_to_align().size();
        //crate::println!("alloc call size: {size}");
        let ptr = self.next_available_slot(size);
        self.get_taken_mut().push((ptr, size));
        self.taken_organize();
        ptr as *mut u8
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { self.alloc(layout) };
        unsafe {
            ptr.write_bytes(0, layout.pad_to_align().size());
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (start, size) = (ptr as usize, layout.pad_to_align().size());
        let v = self.get_taken_mut();
        if let Some(i) = self.taken_match(start, size) {
            v.remove(i);
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old_size = layout.pad_to_align().size();
        let new_size = Layout::from_size_align(new_size, layout.align())
            .unwrap()
            .pad_to_align()
            .size();

        //crate::println!("from {old_size} to {new_size}");
        if old_size >= new_size {
            if let Some(i) = self.taken_match_start(ptr as usize) {
                self.get_taken_mut()[i] = (ptr as usize, new_size);
            }
            return ptr;
        }
        if unsafe { &*self.taken.get() }.as_ptr() as usize == ptr as usize {
            // The 'taken' member called.  This will be some sort of "fakeout" or something.
            let add_size = self.taken_more_size_available(ptr as usize, new_size);
            if old_size + add_size >= new_size {
                self.get_taken_mut()[0] = (ptr as usize, new_size);
                ptr
            } else {
                let &last = self.get_taken().last().unwrap();
                let good = self.next_valid_memory_ptr(endt(last), new_size);
                self.get_taken_mut()[0] = (good, new_size);
                good as *mut u8
            }
        } else {
            unsafe {
                // So, this is not recommended to do, but until I run into actual issues with this as the cause, I will keep this as is.
                let add_size = self.taken_more_size_available(ptr as usize, new_size);
                if old_size + add_size >= new_size {
                    if let Some(i) = self.taken_match_start(ptr as usize) {
                        self.get_taken_mut()[i] = (ptr as usize, new_size);
                    }
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
