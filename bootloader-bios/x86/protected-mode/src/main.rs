#![no_std]
#![no_main]

mod kernel_loading;
mod long;

use core::{arch::asm, panic::PanicInfo, ptr::null};
use htmos_boot_info::{HTMOSBootInformation, HTMOSEntry};

const REAL_SIZE: usize = 0x4C00;
const PROT_SIZE: usize = 0x0600;
const X64_CHECK: *const u8 = 0xFFFB as *const u8;

#[repr(C)]
pub struct HTMOSBootInfoLong {
    pub magic: u64,
    pub boot_mode: u64,
    pub memory_map_addr: u64,
    pub memory_map_size: u64,
    pub memory_desc_size: u64,
    pub framebuffer_addr: u64,
    pub framebuffer_size: u64,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_pitch: u32,
    pub framebuffer_format: u32,
    pub more_info: u64,
}

static mut BOOT_INFO: *const HTMOSBootInformation = null();
const fn boot_info() -> &'static HTMOSBootInformation {
    unsafe { &*BOOT_INFO }
}
const fn boot_info_x64() -> &'static HTMOSBootInfoLong {
    unsafe { &*(BOOT_INFO as *const HTMOSBootInfoLong) }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    if unsafe { X64_CHECK.read_volatile() == 0xFF } {
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
pub extern "cdecl" fn _start(boot_info: usize) -> ! {
    unsafe {
        BOOT_INFO = boot_info as *const _;
    }

    //const KERNEL_LOAD_ADDR: usize = 0x0001_0000;
    //const KERNEL_SIZE_ADDR: *mut u32 = 0xFFFC as *mut u32;

    if unsafe { X64_CHECK.read_volatile() != 0xFF } {
        let krnl_sz = unsafe { *(0xFFFC as *mut u32) } as usize;
        let kbuf = unsafe { core::slice::from_raw_parts_mut(0x0001_0000 as *mut u8, krnl_sz) };
        let kernel_entry = unsafe { kernel_loading::load_elf32(kbuf) }.unwrap() as usize;

        let kentry: HTMOSEntry = unsafe { core::mem::transmute(kernel_entry as u32) };
        kentry(boot_info as *const HTMOSBootInformation)
    } else {
        long::init();
        long::LONG_MODE_GDT.load();
        enter_long_mode_and_jump_to_stage_4(boot_info)
    }
}

pub fn enter_long_mode_and_jump_to_stage_4(boot_info: usize) -> ! {
    const JUMP_TO: usize = 0x7E00 + REAL_SIZE + PROT_SIZE;
    unsafe {
        asm!(
            // align the stack
            "and esp, 0xffffff00",
            // push arguments (extended to 64 bit)
            "push 0",
            "push {info:e}",
            // push entry point address (extended to 64 bit)
            "push 0",
            "push {entry_point:e}",
            info = in(reg) boot_info as u32,
            entry_point = in(reg) JUMP_TO,
        );
        asm!("ljmp $0x8, $2f", "2:", options(att_syntax));
        asm!(
            ".code64",

            // reload segment registers
            "mov {0}, 0x10",
            "mov ds, {0}",
            "mov es, {0}",
            "mov ss, {0}",

            // jump to 4th stage
            "pop rax",
            "pop rdi",
            
            "call rax",
            "2:",
            "jmp 2b",
            out(reg) _,
            out("rax") _,
            out("rdi") _,
        );

    }
    loop {}
}
