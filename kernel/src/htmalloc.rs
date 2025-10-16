//! **HyperText Markup Allocation System**

extern crate alloc;

use crate::{
    boot_info::boot_info,
    cfg_tbl::{FirmwareTable, LZMA_CUSTOM_DECOMPRESS, guid_utf8_upper},
    print, println,
};
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::Cell,
    ptr::null_mut,
};
use htmos_boot_info::HTMOSBootInformation;
use r_efi::efi::{self, ConfigurationTable, MemoryDescriptor, SystemTable};

const ARENA_SIZE: usize = 128 * 1024;
const MAX_SUPPORTED_ALIGN: usize = 4096;

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
const fn cmp_mem_sct(
    original_start: usize,
    original_size: usize,
    given_start: usize,
    given_size: usize,
) -> MemoryPattern {
    if original_start == given_start && original_size == given_size {
        return MemoryPattern::NoChange;
    }

    let original_end = original_start + original_size * 4096;
    let given_end = given_start + given_size * 4096;

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
    taken: Cell<([(usize, usize); 0x1000], usize)>,
}
impl HTMAlloc {
    pub const fn ginit() -> Self {
        Self {
            mmap: Cell::new(([(0, 0); 256], 0)),
            taken: Cell::new(([(0, 0); 0x1000], 0)),
        }
    }

    /// Marks a specified range of memory for free use.
    fn add_range(&self, start: usize, size: usize) {
        if size == 0 {
            return;
        }

        let (mut arr, mut sz) = self.mmap.get();
        if arr[..sz].is_sorted_by(|(v1, _), (v2, _)| v1 <= v2) {
            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
        }

        if sz == 0 {
            arr[0] = (start, size);
        } else if sz == 1 {
            let astart = arr[0].0;
            let asize = arr[0].1;
            match cmp_mem_sct(astart, asize, start, size) {
                MemoryPattern::NoChange => {}
                MemoryPattern::Overwrite => arr[0] = (start, size),
                MemoryPattern::Separate => {
                    arr[1] = (start, size);
                    arr[..2].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
                    sz = 2;
                }
                MemoryPattern::StartBranch => arr[0] = (astart, (start - astart) / 4096 + size),
                MemoryPattern::EndBranch => arr[0] = (start, (astart - start) / 4096 + asize),
            }
        } else {
            if !arr[..sz].contains(&(start, size)) {
                arr[sz] = (start, size);
                sz += 1;
                arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
            }

            loop {
                let mut b = true;

                let mut i = 0;
                for window in arr[..sz].windows(2) {
                    match cmp_mem_sct(window[0].0, window[0].1, window[1].0, window[1].1) {
                        MemoryPattern::NoChange => {
                            arr[i + 1] = (0, 0);
                            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v2.cmp(v1));
                            sz -= 1;
                            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
                            b = false;
                            break;
                        }
                        MemoryPattern::Overwrite => {
                            arr[i] = (0, 0);
                            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v2.cmp(v1));
                            sz -= 1;
                            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
                            b = false;
                            break;
                        }
                        MemoryPattern::Separate => {}
                        MemoryPattern::StartBranch => {
                            arr[i] = (
                                window[0].0,
                                (window[1].0 - window[0].0) / 4096 + window[1].1,
                            );
                            arr[i + 1] = (0, 0);
                            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v2.cmp(v1));
                            sz -= 1;
                            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
                            b = false;
                            break;
                        }
                        MemoryPattern::EndBranch => {
                            arr[i] = (
                                window[1].0,
                                (window[0].0 - window[1].0) / 4096 + window[0].1,
                            );
                            arr[i + 1] = (0, 0);
                            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v2.cmp(v1));
                            sz -= 1;
                            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
                            b = false;
                            break;
                        }
                    }

                    i += 1;
                }

                if b {
                    break;
                }
            }
        }

        // Kinda looks like ReactJS in a way...
        self.mmap.set((arr, sz));
    }
    /// Removes mark of the specified range of memory so it won't be used.  Will panic or block thread if memory is already taken.
    fn remove_range(&self, start: usize, size: usize) {}
    /// This will perform real-time action after the boot information is handled correctly.
    ///
    /// This will go through each memory chunk, parse all config tables given, and mark free as much as possible.
    pub fn update(&self) {
        let bi = boot_info();

        println!("MEM MAP SIZE: {}", bi.memory_map_size);
        println!("DESC SIZE: {}", bi.memory_desc_size);

        // At this point, the following are valid free memory (except for the kernel itself):
        // Loader Code
        // Loader Data
        // Boot Services Code
        // Boot Services Data
        // Conventional

        // Need to look into:
        // Reserved
        // ACPI Reclaim
        // Persistent
        // Unaccepted

        // I'll think about the Runtime Services Code and Data, but the rest will never be touched.

        let mut new_mmap = [(0, 0, 0); 256];

        let mut ptr = bi.memory_map_addr as *const MemoryDescriptor;
        let count = (bi.memory_map_size / bi.memory_desc_size) as usize;
        for i in 0..count {
            let desc = unsafe { &*ptr };

            //if i != 0 && i % 4 == 0 {
            //    println!();
            //}
            //match desc.r#type {
            //    efi::RESERVED_MEMORY_TYPE => {
            //        print!("RES: ");
            //    }
            //    efi::LOADER_CODE => {
            //        print!("LOC: ");
            //    }
            //    efi::LOADER_DATA => {
            //        print!("LOD: ");
            //    }
            //    efi::BOOT_SERVICES_CODE => {
            //        print!("BSC: ");
            //    }
            //    efi::BOOT_SERVICES_DATA => {
            //        print!("BSD: ");
            //    }
            //    efi::RUNTIME_SERVICES_CODE => {
            //        print!("RSC: ");
            //    }
            //    efi::RUNTIME_SERVICES_DATA => {
            //        print!("RSD: ");
            //    }
            //    efi::CONVENTIONAL_MEMORY => {
            //        print!("CON: ");
            //    }
            //    efi::UNUSABLE_MEMORY => {
            //        print!("UNU: ");
            //    }
            //    efi::ACPI_RECLAIM_MEMORY => {
            //        print!("ARE: ");
            //    }
            //    efi::ACPI_MEMORY_NVS => {
            //        print!("AMM: ");
            //    }
            //    efi::MEMORY_MAPPED_IO => {
            //        print!("MMI: ");
            //    }
            //    efi::MEMORY_MAPPED_IO_PORT_SPACE => {
            //        print!("MPS: ");
            //    }
            //    efi::PAL_CODE => {
            //        print!("PAL: ");
            //    }
            //    efi::PERSISTENT_MEMORY => {
            //        print!("PER: ");
            //    }
            //    efi::UNACCEPTED_MEMORY_TYPE => {
            //        print!("UNA: ");
            //    }
            //    _ => {
            //        print!("UKN: ");
            //    }
            //}
            //print!(
            //    "{:7} pages (0x{:16X})   ",
            //    desc.number_of_pages, desc.physical_start
            //);

            if desc.r#type == efi::LOADER_CODE
                || desc.r#type == efi::LOADER_DATA
                || desc.r#type == efi::BOOT_SERVICES_CODE
                || desc.r#type == efi::BOOT_SERVICES_DATA
                || desc.r#type == efi::CONVENTIONAL_MEMORY
            {
                self.add_range(desc.physical_start as usize, desc.number_of_pages as usize);
            }

            new_mmap[i] = (desc.r#type, desc.physical_start, desc.number_of_pages);

            ptr = unsafe { (ptr as *const u8).add(bi.memory_desc_size as usize) }
                as *const MemoryDescriptor;
        }
        //println!();
        //println!("MIN : 0x{min:016X}");
        //println!("MAX : 0x{max:016X}");
        //let mut tbytes = npages * 4096;
        //let mut c = 0;
        //while tbytes > 1024 {
        //    c += 1;
        //    tbytes /= 1024;
        //}
        //println!(
        //    "PAGE COUNT : {npages} ({} bytes -> {} {})",
        //    npages * 4096,
        //    tbytes,
        //    match c {
        //        0 => "bytes",
        //        1 => "KB",
        //        2 => "MB",
        //        3 => "GB",
        //        4 => "TB",
        //        _ => "wow, ok",
        //    }
        //);
        //if tbytes == max {
        //    println!("All memory accounted for!");
        //} else {
        //    println!("Missing memory!");
        //}

        /*
         * Ranges to remove for good:
         * - The Kernel itself
         * - Outstanding pointers (og memory map, boot info)
         * - reserved section given by the pointer value in boot info (raw config from BIOS, SystemTable from UEFI)
         */

        // Kernel
        {
            let kernel_start = unsafe { &crate::__kernel_start as *const u8 as usize };
            let kernel_size =
                ((unsafe { &crate::__kernel_end as *const u8 as usize } - kernel_start) + 0xFFF)
                    / 0x1000;
            self.remove_range(kernel_start, kernel_size);
        }
        // Linker-defined Stack
        {
            let stack_start = unsafe { &crate::__stack_start as *const u8 as usize };
            let stack_size = ((unsafe { &crate::__stack_end as *const u8 as usize } - stack_start)
                + 0xFFF)
                / 0x1000;
            self.remove_range(stack_start, stack_size);
        }
        // Boot Info
        {
            let bi_start = bi as *const _ as usize;
            let bi_size = size_of::<HTMOSBootInformation>();
            self.remove_range(bi_start, bi_size);
        }
        // Framebuffer
        if bi.framebuffer_addr > 0 {
            
        }
        // More Info pointer
        {
            if bi.boot_mode == 1 {

            }
        }

        new_mmap[..count].sort_unstable_by(|v1, v2| v1.1.cmp(&v2.1));

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
