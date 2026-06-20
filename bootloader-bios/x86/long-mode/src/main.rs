#![no_std]
#![no_main]

mod kernel_loading;

use core::panic::PanicInfo;
use htmos_boot_info::{HTMOSBootInformation, HTMOSEntry};

const ADDR_BOOT_INFO: u16 = 0x7C00;
const ADDR_KRNL_SZ: u16 = 0x7C00 + 0x60;

const fn boot_info() -> &'static HTMOSBootInformation {
    unsafe { &*(ADDR_BOOT_INFO as *const HTMOSBootInformation) }
}

pub fn triple_fault() -> ! {
    use core::arch::asm;

    let idtr: [u8; 10] = [0u8; 10];

    unsafe {
        asm!(
            "lidt [{idtr}]",
            "int3",
            idtr = in(reg) idtr.as_ptr(),
            options(noreturn, nostack)
        );
    }
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
pub fn _start() -> ! {
    let krnl_sz = unsafe { *(ADDR_KRNL_SZ as *mut u32) } as usize;
    let kbuf = unsafe { core::slice::from_raw_parts_mut(0x0001_0000 as *mut u8, krnl_sz) };
    let kernel_entry = unsafe { kernel_loading::load_elf64(kbuf) }.unwrap() as usize;

    let kentry: HTMOSEntry = unsafe { core::mem::transmute(kernel_entry) };
    kentry(ADDR_BOOT_INFO as *const HTMOSBootInformation)
}
