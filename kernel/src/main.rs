#![no_std]
#![no_main]

// SAFETY: given from linker.
unsafe extern "C" {
    static __kernel_start: u8;
    static __kernel_end: u8;
    static __stack_start: u8;
    static __stack_end: u8;
}

#[cfg(target_arch = "x86_64")]
global_asm!(include_str!("./asm_entry_stub/x86_64.s"));
#[cfg(target_arch = "x86")]
global_asm!(include_str!("./asm_entry_stub/x86.s"));
#[cfg(target_arch = "aarch64")]
global_asm!(include_str!("./asm_entry_stub/aarch64.s"));
#[cfg(target_arch = "arm")]
global_asm!(include_str!("./asm_entry_stub/arm.s"));
#[cfg(target_arch = "riscv64")]
global_asm!(include_str!("./asm_entry_stub/riscv64.s"));
#[cfg(target_arch = "riscv32")]
global_asm!(include_str!("./asm_entry_stub/riscv32.s"));

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryPattern {
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

pub const fn end(start: usize, size: usize) -> usize {
    start + size
}
pub const fn endt(tuple: (usize, usize)) -> usize {
    tuple.0 + tuple.1
}

/// Start branch means first (original) set starts first.
pub const fn cmp_mem_sct(
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

extern crate alloc;

mod api;
mod boot_info;
mod cfg_tbl;
mod htmalloc;
mod kiss;

use crate::htmalloc::HTMAlloc;
use crate::kiss::RGB;
use crate::kiss::draw::{Point, Rect};
use crate::{boot_info::boot_info, kiss::draw};
use alloc::vec;
use core::arch::global_asm;
use htmos_boot_info::HTMOSBootInformation;
use r_efi::efi::{self, ConfigurationTable, MemoryDescriptor, RuntimeServices, SystemTable};

#[global_allocator]
static HTMAS: HTMAlloc = HTMAlloc::ginit();

trait MemoryGlue {
    fn glue_section(&mut self, start: usize, size: usize);
    fn rip_section(&mut self, start: usize, size: usize);
    fn organize(&mut self) -> bool;
}
impl<const T: usize> MemoryGlue for ([(usize, usize); T], usize) {
    fn glue_section(&mut self, start: usize, size: usize) {
        if size == 0 {
            return;
        }

        let &mut (mut arr, mut sz) = self;
        if arr[..sz].is_sorted_by(|(v1, _), (v2, _)| v1 <= v2) {
            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
        }

        if sz == 0 {
            arr[0] = (start, size);
            sz = 1;
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
                MemoryPattern::StartBranch => arr[0] = (astart, start - astart + size),
                MemoryPattern::EndBranch => arr[0] = (start, astart - start + asize),
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

        *self = (arr, sz);
    }
    fn rip_section(&mut self, start: usize, size: usize) {
        let &mut (mut arr, mut sz) = self;

        // Check if it's even marked available.
        if arr
            .iter()
            .any(|(v1, v2)| cmp_mem_sct(start, size, *v1, *v2) != MemoryPattern::Separate)
        {
            let mut edit = false;
            let mut i = 0;
            while i < sz {
                match cmp_mem_sct(arr[i].0, arr[i].1, start, size) {
                    MemoryPattern::StartBranch => {
                        edit = true;
                        arr[i].1 = start - arr[i].0;
                    }
                    MemoryPattern::EndBranch => {
                        arr[i] = (end(start, size), endt(arr[i]) - end(start, size));
                        // Original section goes past the given: no need to move on.
                        break;
                    }
                    MemoryPattern::NoChange => {
                        if edit == true {
                            // Memory map is janked up?
                            if self.organize() {
                                // Yup.  Restart.
                                i = 0;
                                continue;
                            } else {
                                unreachable!(
                                    "NoChange memory comparison when an edit in remove comparisons happened previously."
                                );
                            }
                        }

                        // Four scenarios:
                        if arr[i].1 == size {
                            // Remove complete section
                            arr[i] = (0, 0);
                            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v2.cmp(v1));
                            sz -= 1;
                            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
                        } else if arr[i].0 == start {
                            arr[i] = (end(start, size), arr[i].1 - size);
                        } else if endt(arr[i]) == end(start, size) {
                            arr[i] = (arr[i].0, arr[i].1 - size);
                        } else {
                            // Josh is the best (Elaina said so).

                            // This gets complicated: we have to split the section into two sections.
                            // Since this involves reoganizing the map, we'll call that and directly return the function.
                            arr[sz] = (end(start, size), endt(arr[i]) - end(start, size));
                            sz += 1;
                            arr[i] = (arr[i].0, start - arr[i].0);
                            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
                            return;
                        }

                        // This is a special "one and done" case.  Exit the loop.
                        break;
                    }
                    MemoryPattern::Overwrite => {
                        // Explanation at end of this branch
                        edit = true;
                        let t = arr[i];

                        // Completely overwrites the section: remove complete section
                        arr[i] = (0, 0);
                        arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v2.cmp(v1));
                        sz -= 1;
                        arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));

                        // If the calculated end for both sections are equal, no need to move on: end of given section.
                        // Otherwise, given section continues pass current section: move on.
                        if start + size == t.0 + t.1 {
                            break;
                        }
                    }
                    MemoryPattern::Separate => {
                        // If an edit occured, there is no reason to move on: we passed the given section completely.
                        if edit {
                            break;
                        }
                    }
                }

                i += 1;
            }
        }

        *self = (arr, sz);
    }
    fn organize(&mut self) -> bool {
        let &mut (mut arr, mut sz) = self;
        if arr[..sz].is_sorted_by(|(v1, _), (v2, _)| v1 <= v2) {
            arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
        }

        let mut ret = false;
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
                        ret = true;
                        break;
                    }
                    MemoryPattern::Overwrite => {
                        arr[i] = (0, 0);
                        arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v2.cmp(v1));
                        sz -= 1;
                        arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
                        b = false;
                        ret = true;
                        break;
                    }
                    MemoryPattern::Separate => {}
                    MemoryPattern::StartBranch => {
                        arr[i] = (window[0].0, window[1].0 - window[0].0 + window[1].1);
                        arr[i + 1] = (0, 0);
                        arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v2.cmp(v1));
                        sz -= 1;
                        arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
                        b = false;
                        ret = true;
                        break;
                    }
                    MemoryPattern::EndBranch => {
                        arr[i] = (window[1].0, window[0].0 - window[1].0 + window[0].1);
                        arr[i + 1] = (0, 0);
                        arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v2.cmp(v1));
                        sz -= 1;
                        arr[..sz].sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
                        b = false;
                        ret = true;
                        break;
                    }
                }

                i += 1;
            }

            if b {
                break;
            }
        }

        *self = (arr, sz);
        ret
    }
}

/// Gives a table of available memory, only scanning Loader, Boot Service, and Conventional sections.
pub(crate) fn get_mmap() -> ([(usize, usize); 256], usize) {
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

    let mut mmap = ([(0, 0); 256], 0);

    // SAFETY: given the bootloader does its job; otherwise, not safe.
    let mut ptr = bi.memory_map_addr as *const MemoryDescriptor;
    let count = (bi.memory_map_size / bi.memory_desc_size) as usize;
    for _ in 0..count {
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
            mmap.glue_section(
                desc.physical_start as usize,
                desc.number_of_pages as usize * 4096,
            );
        }

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
        let kernel_size = unsafe { &crate::__kernel_end as *const u8 as usize } - kernel_start;
        mmap.rip_section(kernel_start, kernel_size);
    }
    // Linker-defined Stack
    {
        let stack_start = unsafe { &crate::__stack_start as *const u8 as usize };
        let stack_size = unsafe { &crate::__stack_end as *const u8 as usize } - stack_start;
        mmap.rip_section(stack_start, stack_size);
    }
    // Boot Info
    {
        let bi_start = bi as *const _ as usize;
        let bi_size = size_of::<HTMOSBootInformation>();
        mmap.rip_section(bi_start, bi_size);
    }
    // Framebuffer
    if bi.framebuffer_addr > 0 {
        mmap.rip_section(bi.framebuffer_addr, bi.framebuffer_size);
    }
    // Memory Map
    mmap.rip_section(bi.memory_map_addr, bi.memory_map_size);
    // More Info pointer
    if bi.boot_mode == 1 {
        // SAFETY: UEFI turns in boot_mode as 1: more_info is the pointer to the SystemTable struct.
        let st = bi.more_info as *mut SystemTable;

        // System Table itself
        mmap.rip_section(st as usize, size_of::<SystemTable>());

        let (firmware_vendor, firmware_vendor_len) = {
            let st = unsafe { &mut *st };
            let mut l = 0;
            // SAFETY: literall given.
            while unsafe { st.firmware_vendor.add(l).read() } != 0 {
                l += 1;
            }
            (st.firmware_vendor as usize, l + 1) // The +1 is for the null terminated C string
        };

        mmap.rip_section(firmware_vendor, firmware_vendor_len);

        println!(
            "FIRMWARE REVISION: {}.{}",
            unsafe { &*st }.firmware_revision >> 16,
            unsafe { &*st }.firmware_revision & 0xFFFF
        );
        println!(
            "FIRMWARE VENDER: {}",
            // SAFETY: UEFI firmware_vender is 16-bit-wide string.
            unsafe { widestring::U16CStr::from_ptr_str(firmware_vendor as *mut u16 as *const u16) }
                .display()
        );

        // Config Table
        mmap.rip_section(
            unsafe { &*st }.configuration_table as usize,
            unsafe { &*st }.number_of_table_entries * size_of::<ConfigurationTable>(),
        );

        // Runtime Services
        // SAFETY: same with SystemTable.
        let rs = unsafe { &mut *st }.runtime_services;
        mmap.rip_section(rs as usize, size_of::<RuntimeServices>());

        // Here, we go through each pointer of any kind,
        // get the size of the type of pointer,
        // and remove it's availability mark.
        {
            mmap.rip_section(
                unsafe { &*rs }.convert_pointer as usize,
                size_of::<efi::RuntimeConvertPointer>(),
            );
            mmap.rip_section(
                unsafe { &*rs }.get_next_high_mono_count as usize,
                size_of::<efi::RuntimeGetNextHighMonoCount>(),
            );
            mmap.rip_section(
                unsafe { &*rs }.get_next_variable_name as usize,
                size_of::<efi::RuntimeGetNextVariableName>(),
            );
            mmap.rip_section(
                unsafe { &*rs }.get_time as usize,
                size_of::<efi::RuntimeGetTime>(),
            );
            mmap.rip_section(
                unsafe { &*rs }.get_variable as usize,
                size_of::<efi::RuntimeGetVariable>(),
            );
            mmap.rip_section(
                unsafe { &*rs }.get_wakeup_time as usize,
                size_of::<efi::RuntimeGetWakeupTime>(),
            );
            mmap.rip_section(
                unsafe { &*rs }.query_capsule_capabilities as usize,
                size_of::<efi::RuntimeQueryCapsuleCapabilities>(),
            );
            mmap.rip_section(
                unsafe { &*rs }.query_variable_info as usize,
                size_of::<efi::RuntimeQueryVariableInfo>(),
            );
            mmap.rip_section(
                unsafe { &*rs }.reset_system as usize,
                size_of::<efi::RuntimeResetSystem>(),
            );
            mmap.rip_section(
                unsafe { &*rs }.set_time as usize,
                size_of::<efi::RuntimeSetTime>(),
            );
            mmap.rip_section(
                unsafe { &*rs }.set_variable as usize,
                size_of::<efi::RuntimeSetVariable>(),
            );
            mmap.rip_section(
                unsafe { &*rs }.set_virtual_address_map as usize,
                size_of::<efi::RuntimeSetVirtualAddressMap>(),
            );
            mmap.rip_section(
                unsafe { &*rs }.set_wakeup_time as usize,
                size_of::<efi::RuntimeSetWakeupTime>(),
            );
            mmap.rip_section(
                unsafe { &*rs }.update_capsule as usize,
                size_of::<efi::RuntimeUpdateCapsule>(),
            );
        }
    }
    // I don't have a specified way to include BIOS yet.

    // If the first page is marked available, remove that.
    mmap.rip_section(0, 4096);

    mmap
}

// SAFETY: assembly stub calls this by name directly; don't change the name.
#[unsafe(no_mangle)]
extern "C" fn htmkrnl(info: *const HTMOSBootInformation) -> ! {
    if info.is_null() {
        panic!("no boot info given");
    }
    boot_info::set_boot_info(info);
    let bi = boot_info();
    //(unsafe { &mut *(&mut *((&*info).reserved as *mut SystemTable)).runtime_services }
    //    .reset_system)(RESET_COLD, Status::ABORTED, 0, null_mut());

    kiss::fill_screen(0, 0xFF, 0);
    kiss::fill_screen(0, 0, 0);

    HTMAS.update(get_mmap());

    logo();

    #[cfg(debug_assertions)]
    {
        println!(
            "!\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{{|}}~"
        );
        println!("HTMOS Pre-Alpha v0.2.1");

        println!("Alloc Vec test");
        let mut vtest = vec![1];
        assert_eq!(vtest[0], 1, "vtest[0] != 1");
        vtest.push(1);
        let v1 = vtest[1];
        assert_eq!(vtest[1], 1, "vtest[1] != 1");
        let v0 = vtest[0];
        let sumv = v0 + v1;
        let sum = vtest[0] + vtest[1];
        assert_eq!(
            sumv, 2,
            "sumv : {sumv} != 2 (v0: {v0} - v1: {v1} - vtest[0]: {} - vtest[1]: {})",
            vtest[0], vtest[1]
        );
        assert_eq!(sum, 2, "sum : {sum} != 2");
        assert!(
            vtest.iter().sum::<i32>() == 2,
            "iter sum of {} != 2",
            vtest.iter().sum::<i32>()
        );
        drop(vtest);
        println!("Test passed!");
    }

    draw::draw_line(
        Point { x: 0, y: 0 },
        Point {
            x: bi.framebuffer_width.cast_signed(),
            y: bi.framebuffer_height.cast_signed(),
        },
        1,
        RGB::red(),
    );
    draw::draw_arc(200, 200, 50, 0.0, 180.0, RGB::green());
    draw::draw_ellipse_rotated(500, 500, 100f32, 100f32, 0f32, RGB::blue());
    draw::draw_rounded_rect(Rect::from_ltrb(100, 700, 200, 800), 5, 1, RGB::white());

    kiss::clear_screen();

    logo();
    draw::draw_rounded_rect(
        Rect {
            x: (bi.framebuffer_width / 2 - 300) as i32,
            y: (bi.framebuffer_height / 2 - 25) as i32,
            w: 600,
            h: 50,
        },
        5,
        1,
        RGB::white(),
    );

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

fn logo() {
    use embedded_graphics::{pixelcolor::Rgb888, prelude::RgbColor};
    use tinybmp::Bmp;

    let bi = boot_info();
    let start_x = (bi.framebuffer_width / 2) - (458 / 2);
    let start_y = (bi.framebuffer_height / 2) - (77 / 2) - 100;

    // 458x77
    let image_bytes = include_bytes!("../small.bmp");
    let bmp = Bmp::<Rgb888>::from_slice(image_bytes).unwrap();
    for pixel in bmp.pixels() {
        let color = kiss::RGB::rgb(pixel.1.r(), pixel.1.g(), pixel.1.b());
        kiss::set_pixel(
            pixel.0.x as u32 + start_x,
            pixel.0.y as u32 + start_y,
            color,
        )
        .unwrap();
    }
}
