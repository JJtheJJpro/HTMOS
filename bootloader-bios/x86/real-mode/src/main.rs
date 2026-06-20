#![no_std]
#![no_main]

mod bios;

use crate::bios::FbError;
use core::arch::{asm, global_asm};
use core::fmt::Write;
use core::panic::PanicInfo;
use htmos_boot_info::{HTMOSBootInformation32, HTMOSBootInformation64};

const ADDR_E820_BASE: u16 = 0x1000; // This is temporary
const ADDR_BOOT_INFO: u16 = 0x7C00;
const ADDR_X64: u16 = 0x7C00 + 0x50;
const ADDR_DSKNUM: u16 = 0x7C00 + 0x51;
const ADDR_E820_COUNT: u16 = 0x7C00 + 0x52;
const ADDR_KRNL_SZ: u16 = 0x7C00 + 0x60;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    //write!(bios::Writer, "{info}").unwrap_or_else(|_| bios::print_str("PANIC"));
    bios::print_str("PANIC");
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let disk_num = {
        unsafe {
            let mut r: u8 = 0;
            core::arch::asm!(
                "",
                out("dl") r
            );
            r
        }
    };

    unsafe {
        (ADDR_DSKNUM as *mut u8).write_volatile(disk_num);
    }

    bios::print_str("HTMOS x86 BIOS Bootloader");

    if !bios::enable_a20() {
        panic!("Failed to enabled A20");
    }

    //loop {}

    let mmap = unsafe {
        bios::init_memory_map();
        bios::MemoryMap::read()
    };

    let x64 = unsafe { bios::load_kernel(disk_num as u8).unwrap() };

    //unsafe {
    //    bios::dump_vbe_modes();
    //    loop {}
    //}

    let fb_info = unsafe {
        match bios::init_framebuffer() {
            Ok(v) => v,
            Err(FbError::VbeNotSupported) => panic!("VBE not supported"),
            Err(FbError::BadVbeSignature) => panic!("bad VBE signature"),
            Err(FbError::NoModeListPtr) => panic!("no VBE mode list"),
            Err(FbError::NoModesAtAll) => panic!("no VBE modes found"),
            Err(FbError::NoSuitableMode) => panic!("no 32/24/16bpp linear mode"),
            Err(FbError::ModeSetFailed) => panic!("VBE mode set failed"),
        }
    };

    if x64 {
        unsafe {
            (ADDR_X64 as *mut u8).write_volatile(0xFF);
            (ADDR_BOOT_INFO as *mut HTMOSBootInformation64).write_volatile(
                HTMOSBootInformation64 {
                    magic: u64::from_ne_bytes(*b"HTMLBOOT"),
                    boot_mode: 0,
                    memory_map_addr: 0x0500,
                    memory_map_size: mmap.count() as u64 * size_of::<bios::E820Entry>() as u64,
                    memory_desc_size: size_of::<bios::E820Entry>() as u64,
                    framebuffer_addr: fb_info.addr,
                    framebuffer_size: 0,
                    framebuffer_width: fb_info.width,
                    framebuffer_height: fb_info.height,
                    framebuffer_pitch: fb_info.pitch,
                    framebuffer_format: fb_info.bpp as u32,
                    more_info: 0,
                },
            );
        }
    } else {
        unsafe {
            (ADDR_X64 as *mut u8).write_volatile(0x00);
            (ADDR_BOOT_INFO as *mut HTMOSBootInformation32).write_volatile(
                HTMOSBootInformation32 {
                    magic: u64::from_ne_bytes(*b"HTMLBOOT"),
                    boot_mode: 0,
                    memory_map_addr: 0x0500,
                    memory_map_size: mmap.count() as u32 * size_of::<bios::E820Entry>() as u32,
                    memory_desc_size: size_of::<bios::E820Entry>() as u32,
                    framebuffer_addr: fb_info.addr as u32,
                    framebuffer_size: 0,
                    framebuffer_width: fb_info.width,
                    framebuffer_height: fb_info.height,
                    framebuffer_pitch: fb_info.pitch,
                    framebuffer_format: fb_info.bpp as u32,
                    more_info: 0,
                },
            );
        }
    }

    //const JUMP_TO: usize = 0x7E00 + REAL_SIZE

    //bios::GDT.clear_interrupts_and_load();

    unsafe {
        asm!(
            "jmp {}",
            in(reg) 0x7d00,
            options(noreturn)
        );
    }

    //switch_to_protected_mode(JUMP_TO as *const u8);

    //let protected_mode: extern "C" fn(*const HTMOSBootInformation) -> ! = unsafe { core::mem::transmute(JUMP_TO) };
    //protected_mode(&boot_info);

    //let kentry: extern "cdecl" fn(*const HTMOSBootInformation) -> ! = unsafe { core::mem::transmute(1usize) };
    //kentry(&boot_info)
}