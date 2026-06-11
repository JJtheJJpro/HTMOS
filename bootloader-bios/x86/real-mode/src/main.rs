#![no_std]
#![no_main]

mod bios;

use crate::bios::FbError;
use core::arch::asm;
use core::fmt::Write;
use core::panic::PanicInfo;
use htmos_boot_info::HTMOSBootInformation;

const REAL_SIZE: usize = 0x4C00;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    write!(bios::Writer, "{info}").unwrap_or_else(|_| bios::print_str("PANIC"));
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

    let mut bios_writer = bios::Writer;
    writeln!(bios_writer, "HTMOS x86 BIOS Bootloader").unwrap();
    writeln!(bios_writer, "Disk Number: 0x{disk_num:02X}").unwrap();

    if bios::enable_a20() {
        writeln!(bios_writer, "A20 enabled").unwrap();
    } else {
        panic!("Failed to enabled A20");
    }

    //loop {}

    let mmap = unsafe {
        bios::init_memory_map();
        bios::MemoryMap::read()
    };
    writeln!(bios_writer, "Memory map loaded ({})", mmap.count()).unwrap();
    for mentry in mmap.iter() {
        let base = mentry.base;
        let length = mentry.length;
        let etype = mentry.entry_type;
        let attrs = mentry.attrs;
        write!(
            bios_writer,
            "Entry: 0x{:08X}-0x{:08X}",
            base,
            base + length - 1
        )
        .unwrap();
        match etype {
            1 => write!(bios_writer, " (available) "),
            2 => write!(bios_writer, " (reserved)  "),
            3 => write!(bios_writer, " (acpi)      "),
            4 => write!(bios_writer, " (acpi nvs)  "),
            5 => write!(bios_writer, " (unusuable) "),
            6 => write!(bios_writer, " (disabled)  "),
            _ => write!(bios_writer, " (undefined) "),
        }
        .unwrap();
        if attrs & 0b0001 != 0 {
            if attrs & 0b0010 != 0 {
                write!(bios_writer, ", non-volatile").unwrap();
            }
            if attrs & 0b0100 != 0 {
                write!(bios_writer, ",  slow-access").unwrap();
            }
            if attrs & 0b1000 != 0 {
                write!(bios_writer, ",    error-log").unwrap();
            }
        }
        writeln!(bios_writer, "").unwrap();
    }

    let x64 = unsafe { bios::load_kernel(disk_num as u8).unwrap() };

    //unsafe {
    //    bios::dump_vbe_modes();
    //    loop {}
    //}

    let fb_info = unsafe {
        match bios::init_framebuffer() {
            Ok(v) => {
                let w = v.width;
                let h = v.height;
                let p = v.pitch;
                let f = v.bpp;
                writeln!(
                    bios_writer,
                    "Framebuffer good ({}x{} pitch {} format {})",
                    w, h, p, f
                )
                .unwrap();
                v
            }
            Err(FbError::VbeNotSupported) => panic!("VBE not supported"),
            Err(FbError::BadVbeSignature) => panic!("bad VBE signature"),
            Err(FbError::NoModeListPtr) => panic!("no VBE mode list"),
            Err(FbError::NoModesAtAll) => panic!("no VBE modes found"),
            Err(FbError::NoSuitableMode) => panic!("no 32/24/16bpp linear mode"),
            Err(FbError::ModeSetFailed) => panic!("VBE mode set failed"),
        }
    };

    if x64 {
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
        unsafe {
            (0xFFFB as *mut u8).write_volatile(0xFF);
            (PM_BOOTINFO_STASH as *mut HTMOSBootInfoLong).write_volatile(HTMOSBootInfoLong {
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
            });
        }
    } else {
        unsafe {
            (0xFFFB as *mut u8).write_volatile(0x00);
            (PM_BOOTINFO_STASH as *mut HTMOSBootInformation).write_volatile(HTMOSBootInformation {
                magic: u64::from_ne_bytes(*b"HTMLBOOT"),
                boot_mode: 0,
                memory_map_addr: 0x0500,
                memory_map_size: mmap.count() * size_of::<bios::E820Entry>(),
                memory_desc_size: size_of::<bios::E820Entry>(),
                framebuffer_addr: fb_info.addr as usize,
                framebuffer_size: 0,
                framebuffer_width: fb_info.width,
                framebuffer_height: fb_info.height,
                framebuffer_pitch: fb_info.pitch,
                framebuffer_format: fb_info.bpp as u32,
                more_info: 0,
            });
        }
    }

    const JUMP_TO: usize = 0x7E00 + REAL_SIZE;

    bios::GDT.clear_interrupts_and_load();
    switch_to_protected_mode(JUMP_TO as *const u8);

    //let protected_mode: extern "C" fn(*const HTMOSBootInformation) -> ! = unsafe { core::mem::transmute(JUMP_TO) };
    //protected_mode(&boot_info);

    //let kentry: extern "cdecl" fn(*const HTMOSBootInformation) -> ! = unsafe { core::mem::transmute(1usize) };
    //kentry(&boot_info)
}

const PM_ENTRY_STASH: *mut u32 = 0x7C00 as *mut u32;
const PM_BOOTINFO_STASH: *mut u32 = 0x7D00 as *mut u32;

pub fn switch_to_protected_mode(entry_point: *const u8) -> ! {
    unsafe {
        // Stash args before touching anything
        (PM_ENTRY_STASH as *mut u32).write_volatile(entry_point as u32);

        asm!("cli");
        set_protected_mode_bit();

        asm!("ljmp $0x8, $2f", "2:", options(att_syntax));

        asm!(
            ".code32",
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            "mov esp, 0x06000000",
            // Load stashed values using hardcoded addresses as immediates
            "mov eax, dword ptr [0x7C00]",  // entry_point
            "mov ecx, 0x7D00",  // info
            "push ecx",
            "call eax",
            "5:",
            "jmp 5b",
            out("eax") _,
            out("ecx") _,
        );

        loop {}
    }
}

fn set_protected_mode_bit() -> u32 {
    let mut cr0: u32;
    unsafe {
        asm!("mov {:e}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
    }
    let cr0_protected = cr0 | 1;
    write_cr0(cr0_protected);
    cr0
}

fn write_cr0(val: u32) {
    unsafe { asm!("mov cr0, {:e}", in(reg) val, options(nostack, preserves_flags)) };
}
