//! The UEFI Bootloader for HTMOS.

#![no_std]
#![no_main]

mod helper;

use core::{ptr::null_mut, sync::atomic::Ordering, usize};
use elf::{ElfBytes, endian::AnyEndian};
use htmos_boot_info::{HTMOSBootInformation, HTMOSEntry};
use r_efi::{
    efi::{self, ALLOCATE_ADDRESS, Handle, LOADER_DATA, Status, SystemTable},
    protocols, system,
};

/// UEFI Executable Entry Point
#[unsafe(no_mangle)]
pub extern "C" fn efi_main(h: Handle, st: *mut SystemTable) -> Status {
    unsafe {
        helper::SYS_TBL.store(st, Ordering::Release); // This is basically an initialization for the helper functions.
        helper::HANDLE.store(h, Ordering::Release);

        // UEFI -> Underwear Eating Fried Intelligence

        // This makes the System Table more easily usable in rust.
        let sys_tbl = &mut *st;

        // Boot Services easy access.
        let boot_services = &mut *sys_tbl.boot_services;

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
            *(&mut *gop_handle_ptr),
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
        let gop = &mut *(interface as *mut r_efi::protocols::graphics_output::Protocol);
        let gop_mode = &mut *gop.mode;

        let fb_addr = gop_mode.frame_buffer_base;
        let fb_size = gop_mode.frame_buffer_size;
        let gop_info = &mut *gop_mode.info;
        let pixel_format = gop_info.pixel_format;
        let pitch = gop_info.pixels_per_scan_line;
        let (rw, rh) = (gop_info.horizontal_resolution, gop_info.vertical_resolution);

        uefi_println!("Pass 1 Complete (loaded framebuffer and info)");
        //(boot_services.stall)(1_000_000);

        #[cfg(target_arch = "x86_64")]
        let kernel = helper::load_file(null_mut(), cstr16!("htmkrnl.x64"));
        #[cfg(target_arch = "x86")]
        let kernel = helper::load_file(null_mut(), cstr16!("htmkrnl.x86"));
        let skernel = &mut *kernel;

        uefi_println!("Pass 2 Complete (opened kernel)");
        //(boot_services.stall)(1_000_000);

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

        let sfileinfo = &mut *file_info;
        let mut ksize = sfileinfo.file_size;

        let mut kptr = null_mut();
        let r = (boot_services.allocate_pool)(
            efi::LOADER_DATA,
            (&mut *file_info).file_size as usize,
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

        let kbuf = core::slice::from_raw_parts_mut(kptr as *mut u8, ksize as usize);

        uefi_println!("Pass 3 Complete (read kernel)");

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
                    uefi_println!(
                        "This usually happens when the bootloader couldn't get the needed space for the kernel to load into."
                    );
                    if addr & 0xFFF != 0 {
                        uefi_println!(
                            "Ohhhh just kidding.  The address is misaligned. 0x{addr:016X}"
                        );
                        uefi_println!("Waiting longer...");
                        (boot_services.stall)(8_000_000);
                    }
                    (boot_services.stall)(2_000_000);
                    return status;
                }

                if kernel_ventry >= seg_va && kernel_ventry < seg_va + seg_memsz {
                    kernel_pentry = (addr + kernel_ventry - seg_va) as usize;
                }

                let dest = addr as *mut u8;
                let src = kptr.add(seg_offset as usize);
                core::ptr::copy_nonoverlapping(src, dest as *mut _, seg_filesz as usize);

                if seg_memsz > seg_filesz {
                    let bss_ptr = dest.add(seg_filesz as usize);
                    for i in 0..(seg_memsz - seg_filesz) {
                        *bss_ptr.add(i as usize) = 0;
                    }
                }
            }
        }

        let mut boot_info_ptr: *mut HTMOSBootInformation = null_mut();
        let status = (boot_services.allocate_pool)(
            LOADER_DATA,
            size_of::<HTMOSBootInformation>(),
            &mut boot_info_ptr as *mut _ as *mut *mut core::ffi::c_void,
        );
        if status.is_error() {
            uefi_println!("err alloc pool for boot info: {status:?}");
            (boot_services.stall)(2_000_000);
            return status;
        } else if boot_info_ptr.is_null() {
            uefi_println!("err alloc pool for boot info: null ptr");
            (boot_services.stall)(2_000_000);
            return Status::ABORTED;
        }

        uefi_println!("Pass 4 Complete (kernel ready to run)");

        uefi_println!("boot info addr: 0x{:X}", boot_info_ptr as usize);
        uefi_println!(
            "TO THE USER: After the \"Success?\" text is shown (nothing afterwards), if you see this text for more than a second or two, an internal error occured."
        );
        uefi_println!(
            "             Please make sure you installed HTMOS with the correct settings."
        );
        uefi_println!(
            "             If issue persists, let me know how to duplicate the error (or just tell me the specs of your machine) and I will try to fix it."
        );

        let (mmap, mem_map_size, desc_size) = match helper::exit_boot_services() {
            Ok(v) => v,
            Err(s) => return s,
        };

        (*boot_info_ptr).magic = u64::from_ne_bytes(*b"HTMLBOOT");
        (*boot_info_ptr).boot_mode = 1;
        (*boot_info_ptr).memory_map_addr = mmap as usize;
        (*boot_info_ptr).memory_map_size = mem_map_size;
        (*boot_info_ptr).memory_desc_size = desc_size;
        (*boot_info_ptr).framebuffer_addr = fb_addr as usize;
        (*boot_info_ptr).framebuffer_size = fb_size;
        (*boot_info_ptr).framebuffer_width = rw;
        (*boot_info_ptr).framebuffer_height = rh;
        (*boot_info_ptr).framebuffer_pitch = pitch;
        (*boot_info_ptr).framebuffer_format = pixel_format;
        (*boot_info_ptr).more_info = st as usize;

        let kentry: HTMOSEntry = core::mem::transmute(kernel_pentry);

        kentry(boot_info_ptr);
    }
}
