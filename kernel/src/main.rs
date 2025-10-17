#![no_std]
#![no_main]

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

mod api;
mod boot_info;
mod cfg_tbl;
mod htmalloc;
mod kiss;

use core::arch::global_asm;

use crate::htmalloc::HTMAlloc;
use htmos_boot_info::HTMOSBootInformation;
use r_efi::efi::SystemTable;

#[global_allocator]
static HTMAS: HTMAlloc = HTMAlloc::ginit();

#[unsafe(no_mangle)]
extern "C" fn htmkrnl(info: *const HTMOSBootInformation) -> ! {
    if info.is_null() {
        panic!("no boot info given");
    }
    boot_info::set_boot_info(info);
    //(unsafe { &mut *(&mut *((&*info).reserved as *mut SystemTable)).runtime_services }
    //    .reset_system)(RESET_COLD, Status::ABORTED, 0, null_mut());

    kiss::fill_screen(0, 0xFF, 0);
    kiss::fill_screen(0, 0, 0);

    {
        let bi = boot_info::boot_info();
        if bi.boot_mode == 1 {
            let (firmware_revision, firmware_vendor, firmware_vendor_len) = {
                let st = unsafe { &mut *(bi.more_info as *mut SystemTable) };
                let mut l = 0;
                while unsafe { st.firmware_vendor.add(l).read() } != 0 {
                    l += 1;
                }
                (st.firmware_revision, st.firmware_vendor, l)
            };
        }
    }

    HTMAS.update();

    //println!("!\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{{|}}~");
    //println!("HTMOS Pre-Alpha v0.1.1 WIP");
    //println!("Memory Management: 1%");
    //println!("ACPI: 1%");
    //println!("File Systems: 0%");

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
