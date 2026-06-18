#![no_std]
#![no_main]

mod kernel_loading;
mod long;

use core::{arch::asm, panic::PanicInfo, ptr::null};
use htmos_boot_info::{HTMOSBootInformation32, HTMOSBootInformation64, HTMOSEntry};

const ADDR_BOOT_INFO: u16 = 0x7C00;
const ADDR_X64: u16 = 0x7C00 + 80;
const ADDR_KRNL_SZ: u16 = 0x7C00 + 96;

const fn boot_info() -> &'static HTMOSBootInformation32 {
    unsafe { &*(ADDR_BOOT_INFO as *const HTMOSBootInformation32) }
}
const fn boot_info_x64() -> &'static HTMOSBootInformation64 {
    unsafe { &*(ADDR_BOOT_INFO as *const HTMOSBootInformation64) }
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
    if unsafe { (ADDR_X64 as *const u8).read_volatile() == 0xFF } {
        let info = boot_info_x64();
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
    } else {
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
    }

    loop {}
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub fn _start() -> ! {
    //const KERNEL_LOAD_ADDR: usize = 0x0001_0000;
    //const KERNEL_SIZE_ADDR: *mut u32 = 0xFFFC as *mut u32;

    if unsafe { (ADDR_X64 as *const u8).read_volatile() != 0xFF } {
        let krnl_sz = unsafe { *(ADDR_KRNL_SZ as *mut u32) } as usize;
        let kbuf = unsafe { core::slice::from_raw_parts_mut(0x0001_0000 as *mut u8, krnl_sz) };
        let kernel_entry = unsafe { kernel_loading::load_elf32(kbuf) }.unwrap() as usize;

        let kentry: HTMOSEntry = unsafe { core::mem::transmute(kernel_entry as u32) };
        kentry(boot_info as *const HTMOSBootInformation32)
    } else {
        long::init();
        long::LONG_MODE_GDT.load();

        //enter_long_mode_and_jump_to_stage_4();

        unsafe {
            asm!(
                "jmp {}",
                in(reg) 0x7d80,
                options(noreturn)
            );
        }
    }
}

pub fn enter_long_mode_and_jump_to_stage_4() -> ! {
    unsafe {
        asm!(
            // align the stack
            "and esp, 0xffffff00",
        );
        asm!("ljmp $0x8, $2f", "2:", options(att_syntax));
        asm!(
            ".code64",

            // reload segment registers
            "mov rax, 0x10",
            "mov ds, rax",
            "mov es, rax",
            "mov ss, rax",
            "mov rsp, 0x00007C00",

            "call rax",
            "2:",
            "jmp 2b",
            options(noreturn)
        );
    }
}
