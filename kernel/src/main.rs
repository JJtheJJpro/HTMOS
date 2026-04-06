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
global_asm!(include_str!("./asm_entry_stub/x86.s")); // UNTESTED
#[cfg(target_arch = "aarch64")]
global_asm!(include_str!("./asm_entry_stub/aarch64.s")); // UNTESTED
#[cfg(target_arch = "arm")]
global_asm!(include_str!("./asm_entry_stub/arm.s")); // UNTESTED
#[cfg(target_arch = "riscv64")]
global_asm!(include_str!("./asm_entry_stub/riscv64.s")); // UNTESTED
#[cfg(target_arch = "riscv32")]
global_asm!(include_str!("./asm_entry_stub/riscv32.s")); // UNTESTED

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
mod raml;

use crate::{boot_info::boot_info, htmalloc::HTMAlloc};
use core::{arch::global_asm, ops::Add};
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

fn raw_exit() {
    let bi = boot_info();
    if bi.boot_mode == 0 {
        // bios crap
    } else {
        unsafe {
            ((&mut *(&mut *(bi.more_info as *mut SystemTable)).runtime_services).reset_system)(
                efi::RESET_SHUTDOWN,
                efi::Status::SUCCESS,
                0,
                core::ptr::null_mut(),
            );
        }
    }
}

// SAFETY: actual items from UEFI firmware, assuming it doesn't give wrong information.
/// # ONLY USE IN UEFI MODE!
const fn sliced_uefi_cfg_table() -> &'static [ConfigurationTable] {
    let bi = boot_info();
    unsafe {
        core::slice::from_raw_parts(
            (&mut *(bi.more_info as *mut SystemTable)).configuration_table,
            (&mut *(bi.more_info as *mut SystemTable)).number_of_table_entries,
        )
    }
}

const fn checksum_helper_add(r: *const u8, c: usize) -> u8 {
    let mut ret: u8 = 0;
    let mut i = 0;
    loop {
        ret = ret.wrapping_add(unsafe { r.add(i).read() });
        i += 1;
        if i == c {
            break;
        }
    }
    ret
}

// SAFETY: assembly stub calls this by name directly; don't change the name.
#[unsafe(no_mangle)]
extern "C" fn htmkrnl(info: *const HTMOSBootInformation) -> ! {
    if info.is_null() {
        panic!("no boot info given (boot info can't be set at addres 0x0)");
    }
    kiss::set_krnl_err(0x00);

    boot_info::set_boot_info(info);
    let bi = boot_info();

    kiss::fill_screen(0, 0xFF, 0);
    kiss::fill_screen(0, 0, 0);

    kiss::set_krnl_err(0x02);
    HTMAS.update(get_mmap());
    kiss::set_krnl_err(0x00);

    kiss::clear_screen();

    //logo();

    //for c in sliced_uefi_cfg_table() {
    //    let vguid = c.vendor_guid.as_fields();
    //    println!(
    //        "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
    //        vguid.0,
    //        vguid.1,
    //        vguid.2,
    //        vguid.3,
    //        vguid.4,
    //        vguid.5[0],
    //        vguid.5[1],
    //        vguid.5[2],
    //        vguid.5[3],
    //        vguid.5[4],
    //        vguid.5[5],
    //    );
    //}

    kiss::set_krnl_err(0x10);
    let rsdp = if bi.boot_mode == 0 {
        println!("BIOS MODE");
        unsafe { &*(bi.more_info as *const raw_acpi::rsdp::RootSystemDescriptionPointer) }
    } else {
        println!("UEFI MODE");
        let cfg_table = sliced_uefi_cfg_table();
        let mut ret = 0;
        #[cfg(target_pointer_width = "32")]
        {
            for c in cfg_table {
                if c.vendor_guid
                    == efi::Guid::from_fields(
                        0x8868e871,
                        0xe4f1,
                        0x11d3,
                        0xbc,
                        0x22,
                        &[0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
                    )
                {
                    ret = c.vendor_table as usize;
                } else if c.vendor_guid
                    == efi::Guid::from_fields(
                        0xeb9d2d30,
                        0x2d88,
                        0x11d3,
                        0x9a,
                        0x16,
                        &[0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d],
                    )
                {
                    ret = c.vendor_table as usize;
                    break;
                }
            }
        }
        #[cfg(target_pointer_width = "64")]
        {
            for c in cfg_table {
                if c.vendor_guid
                    == efi::Guid::from_fields(
                        0x8868e871,
                        0xe4f1,
                        0x11d3,
                        0xbc,
                        0x22,
                        &[0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
                    )
                {
                    ret = c.vendor_table as usize;
                    break;
                } else if c.vendor_guid
                    == efi::Guid::from_fields(
                        0xeb9d2d30,
                        0x2d88,
                        0x11d3,
                        0x9a,
                        0x16,
                        &[0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d],
                    )
                {
                    ret = c.vendor_table as usize;
                }
            }
        }
        if ret == 0 {
            panic!("ACPI not found");
        }
        unsafe { &*(ret as *const raw_acpi::rsdp::RootSystemDescriptionPointer) }
    };

    kiss::set_krnl_err(0x11);
    if !rsdp.validate_signature() {
        panic!(
            "RSDP Signature Bad; expected \"RSD PTR \", found \"{}\"",
            unsafe { str::from_utf8_unchecked(&rsdp.signature) }
        )
    } else if rsdp.revision > 0
        && checksum_helper_add(
            rsdp as *const _ as *const _,
            size_of::<raw_acpi::rsdp::RootSystemDescriptionPointer>(),
        ) != 0
    {
        panic!("RSDP Checksum Bad (ACPI 2.0+)")
    } else if rsdp.revision == 0 && checksum_helper_add(rsdp as *const _ as *const _, 20) != 0 {
        panic!("RSDP Checksum Bad (ACPI 1.0)")
    } else if !rsdp.validate_sdt_signature() {
        panic!(
            "RSDT/XSDT Signature Invalid; expected either \"RSDT\" or \"XSDT\", found \"{}\"",
            unsafe {
                str::from_utf8_unchecked(
                    &(&*(rsdp.xsdt_address as *const raw_acpi::rsdt::RootSystemDescriptionTable))
                        .header
                        .signature,
                )
            }
        )
    }

    kiss::set_krnl_err(0x00);
    println!("RSDP ok");
    println!("OEM ID: \"{}\"", unsafe {
        str::from_utf8_unchecked(&rsdp.oemid)
    });
    println!(
        "Revision: {} (ACPI {})",
        rsdp.revision,
        if rsdp.revision > 0 { "2.0+" } else { "1.0" }
    );
    #[cfg(target_pointer_width = "64")]
    if rsdp.revision == 0 {
        kiss::set_console_foreground_color(kiss::RGB::rgb(0xC0, 0xC0, 0x00));
        println!("NOTE: 64-bit architecture using 32-bit ACPI.");
        kiss::set_console_foreground_color(kiss::RGB::white());
    }

    // The way I'm gonna do this kind of branch is have this only exist in the code if 32-bit.
    // And if it is 32-bit but revision is 0, the first branch will not run; the second will.
    // The reality of it is, the first branch should never run.  Ever.  That's why I mention extreme caution with the warning.
    let mut aml_data = (0, 0, alloc::vec![]);
    kiss::set_krnl_err(0x70);
    unsafe {
        let mut __i = false;
        #[cfg(target_pointer_width = "32")]
        if rsdp.revision > 0 {
            __i = true;

            kiss::set_console_foreground_color(kiss::RGB::black());
            kiss::set_console_background_color(kiss::RGB::red());
            println!(
                "CRITICAL WARNING: 32-bit architecture using 64-bit ACPI, attempting with extreme caution!!!"
            );
            kiss::set_console_foreground_color(kiss::RGB::white());
            kiss::set_console_background_color(kiss::RGB::black());
        }
        if size_of::<usize>() * 8 == 64 || rsdp.revision == 0 {
            __i = true;

            let sz = if rsdp.revision == 0 {
                (&*(rsdp.rsdt_address as usize
                    as *const raw_acpi::rsdt::RootSystemDescriptionTable))
                    .entry()
                    .len()
            } else {
                (&*(rsdp.xsdt_address as usize
                    as *const raw_acpi::xsdt::ExtendedSystemDescriptionTable))
                    .entry()
                    .len()
            };

            for i in 0..sz {
                let ptr = if rsdp.revision == 0 {
                    (&*(rsdp.rsdt_address as usize
                        as *const raw_acpi::rsdt::RootSystemDescriptionTable))
                        .entry()[i] as usize
                } else {
                    (&*(rsdp.xsdt_address as usize
                        as *const raw_acpi::xsdt::ExtendedSystemDescriptionTable))
                        .entry()[i] as usize
                };

                let sign =
                    str::from_utf8_unchecked(core::slice::from_raw_parts(ptr as *const u8, 4));

                match sign {
                    "SSDT" => {
                        //{
                        //    let _t = (&*(ptr
                        //        as *const raw_acpi::ssdt::SecondarySystemDescriptionTable))
                        //        .header;
                        //    println!(
                        //        "SSDT ({}, {}, {}, {}, {}, {}, {}, {})",
                        //        ((ptr + 4) as *const u32).read_unaligned(),
                        //        _t.revision,
                        //        _t.checksum,
                        //        str::from_utf8_unchecked(&_t.oemid),
                        //        str::from_utf8_unchecked(&_t.oem_table_id),
                        //        ((ptr + 0x18) as *const u32).read_unaligned(),
                        //        ((ptr + 0x1C) as *const u32).read_unaligned(),
                        //        ((ptr + 0x20) as *const u32).read_unaligned(),
                        //    );
                        //}
                        aml_data.2.push(
                            (&*(ptr as *const raw_acpi::ssdt::SecondarySystemDescriptionTable))
                                .def_block(),
                        );
                        continue;
                    }
                    "DSDT" => {
                        if aml_data.0 == 0 {
                            aml_data.0 = (&*(ptr
                                as *const raw_acpi::dsdt::DifferentiatedSystemDescriptionTable))
                                .def_block()
                                .as_ptr() as usize;
                            aml_data.1 = (&*(ptr
                                as *const raw_acpi::dsdt::DifferentiatedSystemDescriptionTable))
                                .def_block()
                                .len();
                        }

                        continue;
                    }
                    "FACP" => {
                        if (&*(ptr as *const raw_acpi::fadt::FixedACPIDescriptionTable)).x_dsdt != 0
                        {
                            aml_data.0 = (&*((&*(ptr
                                as *const raw_acpi::fadt::FixedACPIDescriptionTable))
                                .x_dsdt as usize
                                as *const raw_acpi::dsdt::DifferentiatedSystemDescriptionTable))
                                .def_block()
                                .as_ptr() as usize;
                            aml_data.1 = (&*((&*(ptr
                                as *const raw_acpi::fadt::FixedACPIDescriptionTable))
                                .x_dsdt as usize
                                as *const raw_acpi::dsdt::DifferentiatedSystemDescriptionTable))
                                .def_block()
                                .len();
                        } else {
                            aml_data.0 = (&*((&*(ptr
                                as *const raw_acpi::fadt::FixedACPIDescriptionTable))
                                .dsdt as usize
                                as *const raw_acpi::dsdt::DifferentiatedSystemDescriptionTable))
                                .def_block()
                                .as_ptr() as usize;
                            aml_data.1 = (&*((&*(ptr
                                as *const raw_acpi::fadt::FixedACPIDescriptionTable))
                                .dsdt as usize
                                as *const raw_acpi::dsdt::DifferentiatedSystemDescriptionTable))
                                .def_block()
                                .len();
                        }
                    }
                    &_ => {}
                }

                println!("{sign}");
            }
        }
        if !__i {
            unreachable!(
                "The only way the code reached here is if the architecture is 16-bit...or something, idk...just stop."
            );
        }
    }
    
    struct GHandler;
    impl stable_aml::Handler for GHandler {
        fn read_u8(&self, address: usize) -> u8 {
            unsafe { (address as *const u8).read() }
        }
        fn read_u16(&self, address: usize) -> u16 {
            unsafe { (address as *const u16).read() }
        }
        fn read_u32(&self, address: usize) -> u32 {
            unsafe { (address as *const u32).read() }
        }
        fn read_u64(&self, address: usize) -> u64 {
            unsafe { (address as *const u64).read() }
        }

        fn write_u8(&mut self, address: usize, value: u8) {
            unsafe { (address as *mut u8).write(value) }
        }
        fn write_u16(&mut self, address: usize, value: u16) {
            unsafe { (address as *mut u16).write(value) }
        }
        fn write_u32(&mut self, address: usize, value: u32) {
            unsafe { (address as *mut u32).write(value) }
        }
        fn write_u64(&mut self, address: usize, value: u64) {
            unsafe { (address as *mut u64).write(value) }
        }

        fn read_io_u8(&self, port: u16) -> u8 {
            unsafe { x86_64::instructions::port::Port::<u8>::new(port).read() }
        }
        fn read_io_u16(&self, port: u16) -> u16 {
            unsafe { x86_64::instructions::port::Port::<u16>::new(port).read() }
        }
        fn read_io_u32(&self, port: u16) -> u32 {
            unsafe { x86_64::instructions::port::Port::<u32>::new(port).read() }
        }

        fn write_io_u8(&self, port: u16, value: u8) {
            unsafe {
                x86_64::instructions::port::Port::<u8>::new(port).write(value);
            }
        }
        fn write_io_u16(&self, port: u16, value: u16) {
            unsafe {
                x86_64::instructions::port::Port::<u16>::new(port).write(value);
            }
        }
        fn write_io_u32(&self, port: u16, value: u32) {
            unsafe {
                x86_64::instructions::port::Port::<u32>::new(port).write(value);
            }
        }

        fn read_pci_u8(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u8 {
            let shift = (offset & 3) * 8;
            ((self.read_pci_u32(segment, bus, device, function, offset) >> shift) & 0xFF) as u8
        }
        fn read_pci_u16(
            &self,
            segment: u16,
            bus: u8,
            device: u8,
            function: u8,
            offset: u16,
        ) -> u16 {
            let shift = (offset & 2) * 8;
            ((self.read_pci_u32(segment, bus, device, function, offset) >> shift) & 0xFFFF) as u16
        }
        fn read_pci_u32(
            &self,
            segment: u16,
            bus: u8,
            device: u8,
            function: u8,
            offset: u16,
        ) -> u32 {
            if segment != 0 {
                todo!("segment value of nonzero is not yet implemented.");
            }

            let aligned = offset & !3;

            let address = (1u32 << 31)
                | ((bus as u32) << 16)
                | ((device as u32) << 11)
                | ((function as u32) << 8)
                | (aligned as u32 & 0xFC);

            let mut addr_port = x86_64::instructions::port::Port::<u32>::new(0xCF8);
            let mut data_port = x86_64::instructions::port::Port::<u32>::new(0xCFC);

            unsafe {
                addr_port.write(address);
                data_port.read()
            }
        }

        fn write_pci_u8(
            &self,
            segment: u16,
            bus: u8,
            device: u8,
            function: u8,
            offset: u16,
            value: u8,
        ) {
            let shift = (offset & 3) * 8;
            let mask = !(0xFF << shift);

            let old = self.read_pci_u32(segment, bus, device, function, offset);
            let new = (old & mask) | ((value as u32) << shift);

            self.write_pci_u32(segment, bus, device, function, offset, new);
        }
        fn write_pci_u16(
            &self,
            segment: u16,
            bus: u8,
            device: u8,
            function: u8,
            offset: u16,
            value: u16,
        ) {
            let shift = (offset & 2) * 8;
            let mask = !(0xFFFF << shift);

            let old = self.read_pci_u32(segment, bus, device, function, offset);
            let new = (old & mask) | ((value as u32) << shift);

            self.write_pci_u32(segment, bus, device, function, offset, new);
        }
        fn write_pci_u32(
            &self,
            segment: u16,
            bus: u8,
            device: u8,
            function: u8,
            offset: u16,
            value: u32,
        ) {
            if segment != 0 {
                todo!("segment value of nonzero is not yet implemented.");
            }

            let aligned = offset & !3;

            let address = (1u32 << 31)
                | ((bus as u32) << 16)
                | ((device as u32) << 11)
                | ((function as u32) << 8)
                | (aligned as u32 & 0xFC);

            let mut addr_port = x86_64::instructions::port::Port::<u32>::new(0xCF8);
            let mut data_port = x86_64::instructions::port::Port::<u32>::new(0xCFC);

            unsafe {
                addr_port.write(address);
                data_port.write(value)
            }
        }
    }
    
    let mut aml = stable_aml::AmlContext::new(alloc::boxed::Box::new(GHandler), stable_aml::DebugVerbosity::All);
    
    if aml_data.0 != 0 {
        kiss::set_krnl_err(0x71);
        unsafe {
            if let Err(e) = aml.parse_table(core::slice::from_raw_parts(
                aml_data.0 as *const u8,
                aml_data.1,
            )) {
                panic!("{e:?}");
            }
        }
    }
    let mut err = 0;
    kiss::set_krnl_err(0x72);
    for ssdt in aml_data.2 {
        if let Err(_) = aml.parse_table(ssdt) {
            err += 1;
        }
    }

    kiss::set_krnl_err(0x00);

    println!("{err} out of {} SSDTs parsed incorrectly", aml_data.1);
    

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
    let start_y = (bi.framebuffer_height / 2) - (77 / 2); // - 100;

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
