//! The UEFI Bootloader for HTMOS.

#![no_std]
#![no_main]

mod bootinfo;
mod helper;

use bootinfo::BootInfo;
use core::{ptr::null_mut, sync::atomic::Ordering, u64, usize};
use elf::{ElfBytes, endian::AnyEndian};
use r_efi::{
    efi::{self, ALLOCATE_ADDRESS, Handle, LOADER_DATA, Status, SystemTable},
    protocols, system,
};

/// UEFI Executable Entry Point
#[unsafe(no_mangle)]
pub extern "C" fn efi_main(h: Handle, st: *mut SystemTable) -> Status {
    helper::SYS_TBL.store(st, Ordering::Release); // This is basically an initialization for the helper functions.
    helper::HANDLE.store(h, Ordering::Release);

    // UEFI -> Underwear Eating Fried Intelligence

    // This makes the System Table more easily usable in rust.
    let sys_tbl = unsafe { &mut *st };

    // Boot Services easy access.
    let boot_services = unsafe { &mut *sys_tbl.boot_services };

    uefi_println!("HTMOS Official UEFI Bootloader");

    // Graphics Output Protocol process (to get raw framebuffer and settings)
    let mut n_gop_handles = 0;
    let mut gop_handle_ptr: *mut Handle = null_mut();
    let r = (boot_services.locate_handle_buffer)(
        system::BY_PROTOCOL,
        &r_efi::protocols::graphics_output::PROTOCOL_GUID as *const _ as *mut _,
        null_mut(),
        &mut n_gop_handles,
        &mut gop_handle_ptr,
    );
    if r != Status::SUCCESS {
        uefi_println!("locate_handle_buffer failed: {r:?}");
        (boot_services.stall)(2_000_000);
        return r;
    }
    if n_gop_handles > 1 {
        // Oh wow.  More than one GOP handles???  uwu
        uefi_println!("n_gop_handles: {n_gop_handles}");
    }
    let mut interface: Handle = null_mut();
    let r = (boot_services.open_protocol)(
        *(&mut unsafe { *gop_handle_ptr }),
        &r_efi::protocols::graphics_output::PROTOCOL_GUID as *const _ as *mut _,
        &mut interface,
        h,
        null_mut(),
        system::OPEN_PROTOCOL_GET_PROTOCOL,
    );
    if r != Status::SUCCESS {
        uefi_println!("open_protocol failed: 0x{:016X}", r.as_usize());
        (boot_services.stall)(2_000_000);
        return r;
    }
    let gop = unsafe { &mut *(interface as *mut r_efi::protocols::graphics_output::Protocol) };
    let gop_mode = unsafe { &mut *gop.mode };

    let fb_addr = gop_mode.frame_buffer_base;
    let _fb_size = gop_mode.frame_buffer_size; // I'll probably need this later...
    let gop_info = unsafe { &mut *gop_mode.info };
    let pixel_format = gop_info.pixel_format;
    let pitch = gop_info.pixels_per_scan_line;
    let (rw, rh) = (gop_info.horizontal_resolution, gop_info.vertical_resolution);

    uefi_println!("Pass 1 Complete (loaded framebuffer and info)");
    (boot_services.stall)(1_000_000);

    let kernel = helper::load_file(null_mut(), cstr16!("htmkrnl"));
    let skernel = unsafe { &mut *kernel };

    uefi_println!("Pass 2 Complete (opened kernel)");
    (boot_services.stall)(1_000_000);

    let mut file_info_size = 0;
    let mut file_info: *mut protocols::file::Info = null_mut();
    let r = (skernel.get_info)(
        kernel,
        &efi::protocols::file::INFO_ID as *const _ as *mut _,
        &mut file_info_size,
        null_mut(),
    );
    if r != Status::BUFFER_TOO_SMALL {
        uefi_println!("fileinfo fail 1");
        (boot_services.stall)(2_000_000);
        return r;
    }

    let r = (boot_services.allocate_pool)(
        efi::LOADER_DATA,
        file_info_size,
        &mut file_info as *mut _ as *mut _,
    );
    if r != Status::SUCCESS {
        uefi_println!("allocatepool fileinfo fail");
        (boot_services.stall)(2_000_000);
        return r;
    }

    let r = (skernel.get_info)(
        kernel,
        &efi::protocols::file::INFO_ID as *const _ as *mut _,
        &mut file_info_size,
        file_info as *mut _,
    );
    if r != Status::SUCCESS {
        uefi_println!("fileinfo fail 2");
        (boot_services.stall)(2_000_000);
        return r;
    }

    let sfileinfo = unsafe { &mut *file_info };
    let mut ksize = sfileinfo.file_size;

    let mut kptr = null_mut();
    let r = (boot_services.allocate_pool)(
        efi::LOADER_DATA,
        unsafe { &mut *file_info }.file_size as usize,
        &mut kptr as *mut _,
    );
    if r != Status::SUCCESS {
        uefi_println!("allocatepool fileinfo fail");
        (boot_services.stall)(2_000_000);
        return r;
    }

    let r = (skernel.read)(kernel, &mut ksize as *mut _ as *mut _, kptr);
    if r != Status::SUCCESS {
        uefi_println!("kernel read fail");
        (boot_services.stall)(2_000_000);
        return r;
    }

    let kbuf = unsafe { core::slice::from_raw_parts_mut(kptr as *mut u8, ksize as usize) };

    uefi_println!("Pass 3 Complete (read kernel)");
    (boot_services.stall)(1_000_000);

    let elf_kernel = ElfBytes::<AnyEndian>::minimal_parse(kbuf).unwrap();
    let kernel_ventry = elf_kernel.ehdr.e_entry;
    let mut kernel_pentry = 0;
    for ph in elf_kernel.segments().unwrap() {
        if ph.p_type == elf::abi::PT_LOAD {
            let seg_va = ph.p_vaddr;
            let seg_memsz = ph.p_memsz;
            let seg_filesz = ph.p_filesz;
            let seg_offset = ph.p_offset;
            let page_count = (seg_memsz + 0xFFF) / 0x1000;

            let mut addr = seg_va as efi::PhysicalAddress;
            let status = (boot_services.allocate_pages)(
                ALLOCATE_ADDRESS,
                LOADER_DATA,
                page_count as usize,
                &mut addr,
            );
            if status != Status::SUCCESS {
                uefi_println!("kernel put in memory failed 0x{:016X}", status.as_usize());
                uefi_println!("This usually happens when the bootloader couldn't get the needed space for the kernel to load into.");
                if addr & 0xFFF != 0 {
                    uefi_println!("Ohhhh just kidding.  The address is misaligned. 0x{addr:016X}");
                    uefi_println!("Waiting longer...");
                    (boot_services.stall)(8_000_000);
                }
                (boot_services.stall)(2_000_000);
                return status;
            }

            if kernel_ventry >= seg_va && kernel_ventry < seg_va + seg_memsz {
                kernel_pentry = addr + kernel_ventry - seg_va;
            }

            let dest = addr as *mut u8;
            let src = unsafe { kptr.add(seg_offset as usize) };
            unsafe {
                core::ptr::copy_nonoverlapping(src, dest as *mut _, seg_filesz as usize);
            }

            if seg_memsz > seg_filesz {
                let bss_ptr = unsafe { dest.add(seg_filesz as usize) };
                for i in 0..(seg_memsz - seg_filesz) {
                    unsafe {
                        *bss_ptr.add(i as usize) = 0;
                    }
                }
            }
        }
    }

    let (mmap, mem_map_size, desc_size) = match helper::exit_boot_services() {
        Ok(v) => v,
        Err(s) => return s,
    };

    let kentry: extern "C" fn(*const BootInfo) = unsafe { core::mem::transmute(kernel_pentry) };

    let asdf = BootInfo {
        memory_map_addr: mmap as u64,
        memory_map_len: mem_map_size as u64,
        memory_desc_size: desc_size as u64,
        framebuffer_addr: fb_addr,
        framebuffer_width: rw,
        framebuffer_height: rh,
        framebuffer_pitch: pitch,
        framebuffer_bpp: pixel_format,
        boot_mode: 1,
        reserved: st as u32,
    };

    kentry(&asdf as *const BootInfo);

    /*

    // Parsing the santa slave
    let mut buf = [0u8; 1024 * 20]; // 20 KB
    let mut size = buf.len();
    let r = unsafe { ((*kernel).read)(kernel, &mut size, buf.as_mut_ptr() as *mut c_void) };
    if r != Status::SUCCESS {
        uefi_println!("KernelRead1 Fail");
        (boot_services.stall)(2_000_000);
        return r;
    }

    uefi_println!("Pass 4 Complete (read kernel)");
    (boot_services.stall)(1_000_000);

    let elf = ElfFile::new(&buf).unwrap();

    // For loading the kernel, we first need to find and get the memory foorprint of the kernel...
    let mut min_vaddr = u64::MAX;
    let mut max_vaddr = u64::MIN;
    for ph in elf.program_iter() {
        if ph.get_type() == Ok(xmas_elf::program::Type::Load) {
            min_vaddr = min_vaddr.min(ph.virtual_addr());
            max_vaddr = max_vaddr.max(ph.virtual_addr() + ph.mem_size());
        }
    }

    let kernel_size = max_vaddr - min_vaddr;
    let num_pages = (kernel_size + 0xFFF) / 0x1000;

    let mut kernel_phys_addr: efi::PhysicalAddress = 0;
    let r = (boot_services.allocate_pages)(
        efi::ALLOCATE_ANY_PAGES,
        efi::LOADER_CODE,
        num_pages as usize,
        &mut kernel_phys_addr,
    );
    if r != Status::SUCCESS {
        uefi_println!("AllocatePages for kernel Fail");
        (boot_services.stall)(2_000_000);
        return r;
    }

    for ph in elf.program_iter() {
        if ph.get_type() == Ok(xmas_elf::program::Type::Load) {
            let segment_offset_in_file = ph.offset() as usize;
            let segment_file_size = ph.file_size() as usize;
            let segment_mem_size = ph.mem_size() as usize;
            let segment_vaddr = ph.virtual_addr();

            // Calculate the physical destination for this segment
            let dest_addr = kernel_phys_addr + (segment_vaddr - min_vaddr);

            unsafe {
                // Copy the segment from the file buffer to the allocated physical memory
                core::ptr::copy_nonoverlapping(
                    buf.as_ptr().add(segment_offset_in_file),
                    dest_addr as *mut u8,
                    segment_file_size,
                );

                // If mem_size > file_size, this is the .bss section. Zero it.
                if segment_mem_size > segment_file_size {
                    core::ptr::write_bytes(
                        (dest_addr as usize + segment_file_size) as *mut u8,
                        0,
                        segment_mem_size - segment_file_size,
                    );
                }
            }
        }
    }

    uefi_println!("Pass 5 Complete (copied nonoverlapping data)");
    (boot_services.stall)(1_000_000);

    let entry_offset = elf.header.pt2.entry_point() - min_vaddr;
    let kernel_entry = kernel_phys_addr + entry_offset;

    uefi_print!("Kernel loaded.  Exiting boot services in 3");
    (boot_services.stall)(1_000_000);
    uefi_print!("\rKernel loaded.  Exiting boot services in 2");
    (boot_services.stall)(1_000_000);
    uefi_print!("\rKernel loaded.  Exiting boot services in 1");
    (boot_services.stall)(1_000_000);

    (boot_services.free_pool)(gop_handle_ptr as *mut c_void);

    let (mmap, mem_map_size, _desc_size) = helper::exit_boot_services().unwrap();

    let fb = fb_addr as *mut u32;
    for y in 0..rh {
        for x in 0..rw {
            unsafe {
                *fb.add((y * rw + x) as usize) = 0x0000FF;
            }
        }
    }

    let kentry: extern "C" fn(*const BootInfo) = unsafe { core::mem::transmute(kernel_entry) };

    for y in 0..rh {
        for x in 0..rw {
            unsafe {
                *fb.add((y * rw + x) as usize) = 0x000000;
            }
        }
    }

    let asdf = BootInfo {
        memory_map_addr: mmap as u64,
        memory_map_len: mem_map_size as u64,
        framebuffer_addr: fb_addr,
        framebuffer_width: rw,
        framebuffer_height: rh,
        framebuffer_pitch: pitch,
        framebuffer_bpp: pixel_format,
        boot_mode: 1,
        reserved: 0,
    };

    kentry(&asdf as *const BootInfo);

    unsafe {
        ((&mut *sys_tbl.runtime_services).reset_system)(
            RESET_SHUTDOWN,
            Status::SUCCESS,
            0,
            null_mut(),
        );
    }

    */

    Status::SUCCESS

    //for y in 0..rh {
    //    for x in 0..rw {
    //        kernel::klib::put_pixel(
    //            fb_addr as *mut u8,
    //            pitch as usize,
    //            x as usize,
    //            y as usize,
    //            [0, 0, 0],
    //            pixel_format,
    //        );
    //    }
    //}

    //let r = kernel::kernel(
    //    mmap,
    //    mem_map_size,
    //    desc_size,
    //    fb_addr,
    //    pitch as usize,
    //    pixel_format,
    //);

    //(unsafe { &mut *(&mut *st).runtime_services }.reset_system)(r.0, r.1, 0, null_mut());
}
