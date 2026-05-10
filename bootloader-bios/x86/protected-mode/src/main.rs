#![no_std]
#![no_main]

mod kernel_loading;

use core::{panic::PanicInfo, ptr::null};
use htmos_boot_info::{HTMOSBootInformation, HTMOSEntry};

static mut BOOT_INFO: *const HTMOSBootInformation = null();
const fn boot_info() -> &'static HTMOSBootInformation {
    unsafe { &*BOOT_INFO }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    let info = boot_info();
    unsafe {
        let fb = info.framebuffer_addr as *mut u8;
        let width = info.framebuffer_width;
        let height = info.framebuffer_height;
        let pitch = info.framebuffer_pitch;
        let bpp = info.framebuffer_format as u32;
        let bytes_per_pixel = (bpp + 7) / 8;
        for y in 0..height {
            for x in 0..width {
                let offset = (y * pitch + x * bytes_per_pixel) as isize;
                let pixel = fb.offset(offset) as *mut u32;
                match bytes_per_pixel {
                    4 => pixel.write_volatile(0x00FF0000), // 32bpp ARGB red
                    3 => {
                        fb.offset(offset + 0).write_volatile(0x00); // B
                        fb.offset(offset + 1).write_volatile(0x00); // G
                        fb.offset(offset + 2).write_volatile(0xFF); // R
                    }
                    2 => (fb.offset(offset) as *mut u16).write_volatile(0xF800), // 16bpp RGB565 red
                    _ => {}
                }
            }
        }
    }
    loop {}
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "cdecl" fn _start(boot_info: usize) -> ! {
    let boot_info_h = unsafe { &*(boot_info as *const HTMOSBootInformation) };
    unsafe {
        BOOT_INFO = boot_info_h;
    }

    const KERNEL_LOAD_ADDR: usize = 0x0001_0000;
    const KERNEL_SIZE_ADDR: *mut u32 = 0xFFFC as *mut u32;

    let krnl_sz = unsafe { *KERNEL_SIZE_ADDR } as usize;
    let kbuf = unsafe { core::slice::from_raw_parts_mut(KERNEL_LOAD_ADDR as *mut u8, krnl_sz) };
    let kernel_entry = unsafe { kernel_loading::load_elf32(kbuf) }.unwrap() as usize;

    #[cfg(target_arch = "x86")]
    {
        let kentry: HTMOSEntry = unsafe { core::mem::transmute(kernel_entry as u32) };
        kentry(boot_info as *const HTMOSBootInformation)
    }
    #[cfg(target_arch = "x86_64")]
    {
        todo!("long mode not implemented yet");
    }
}
