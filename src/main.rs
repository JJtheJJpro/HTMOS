#![no_std]
#![no_main]

mod html;
mod kernel;

use core::{
    ffi::c_void,
    fmt::{Arguments, Write},
    panic::PanicInfo,
    ptr::null_mut,
    sync::atomic::{AtomicPtr, Ordering},
};
use r_efi::{
    efi::{self, Handle, Status, SystemTable},
    system,
};

macro_rules! cstr16 {
    ($s:literal) => {
        $s.encode_utf16()
            .chain(core::iter::once(0))
            .collect::<heapless::Vec<u16, 256>>()
            .as_mut_ptr()
    };
    ($s:expr) => {
        $s.encode_utf16()
            .chain(core::iter::once(0))
            .collect::<heapless::Vec<u16, 256>>()
            .as_mut_ptr()
    };
}
macro_rules! uefi_print {
    ($($arg:tt)*) => {
        crate::print(format_args!($($arg)*));
    };
}
macro_rules! uefi_println {
    () => {
        uefi_print!("\r\n");
    };
    ($($arg:tt)*) => {
        crate::print(format_args!("{}{}", format_args!($($arg)*), "\r\n"));
    };
}

pub(crate) static SYS_TBL: AtomicPtr<SystemTable> = AtomicPtr::new(null_mut());
struct STHolder;
impl Write for STHolder {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let con_out = unsafe { &mut *(&mut *SYS_TBL.load(Ordering::Acquire)).con_out };
        (con_out.output_string)(con_out, cstr16!(s));
        Ok(())
    }
}

#[doc(hidden)]
fn print(args: Arguments) {
    STHolder {}.write_fmt(args).unwrap();
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    if boot_services_alive() {
        uefi_println!("[PANIC]: {info}");
        (unsafe { &mut *(&mut *SYS_TBL.load(Ordering::Acquire)).boot_services }.stall)(5_000_000);
    } else {
        if !kernel::KISS_VAR.load(Ordering::Acquire).is_null() {
            println!("[PANIC]: {info}");
            kernel::pause();
        } else {
            kernel::klib::sleep_ms(5000);
        }
    }

    (unsafe { &mut *(&mut *SYS_TBL.load(Ordering::Acquire)).runtime_services }.reset_system)(
        efi::RESET_SHUTDOWN,
        Status::ABORTED,
        0,
        null_mut(),
    );

    loop {}
}

pub(crate) fn boot_services_alive() -> bool {
    unsafe { &mut *SYS_TBL.load(Ordering::Acquire) }.boot_services != null_mut()
}

#[unsafe(export_name = "efi_main")]
pub extern "C" fn main(h: Handle, st: *mut SystemTable) -> Status {
    let sys_tbl = unsafe { &mut *st };
    SYS_TBL.store(sys_tbl, Ordering::Release);
    let boot_services = unsafe { &mut *sys_tbl.boot_services };

    uefi_println!("Hello from UEFI boot service!");

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
        uefi_println!(
            "open_protocol failed: {:?}",
            r.as_usize() - 0x8000000000000000
        );
        (boot_services.stall)(2_000_000);
        return r;
    }
    let gop = unsafe { &mut *(interface as *mut r_efi::protocols::graphics_output::Protocol) };

    let gop_mode = unsafe { &mut *gop.mode };

    let fb_addr = gop_mode.frame_buffer_base;
    let fb_size = gop_mode.frame_buffer_size;
    let gop_info = unsafe { &mut *gop_mode.info };
    let pixel_format = gop_info.pixel_format;
    let pitch = gop_info.pixels_per_scan_line;
    let (rw, rh) = (gop_info.horizontal_resolution, gop_info.vertical_resolution);

    uefi_println!("0x{fb_addr:016X} {pixel_format} {pitch} {rw}x{rh}");

    uefi_println!("exiting...");
    (boot_services.stall)(1_000_000);
    (boot_services.free_pool)(gop_handle_ptr as *mut c_void);

    let (mmap, mem_map_size, desc_size) = loop {
        let mut mem_map_size = 0;
        let mut mem_map = null_mut();
        let mut map_key = 0;
        let mut desc_size = 0;
        let mut desc_ver = 0;

        let r = (boot_services.get_memory_map)(
            &mut mem_map_size,
            mem_map,
            &mut map_key,
            &mut desc_size,
            &mut desc_ver,
        );
        if r != Status::BUFFER_TOO_SMALL {
            return r;
        }
        mem_map_size += desc_size * 2;
        let r = (boot_services.allocate_pool)(
            efi::LOADER_DATA,
            mem_map_size,
            &mut mem_map as *mut _ as *mut *mut c_void,
        );
        if r != Status::SUCCESS {
            return r;
        }
        uefi_println!("Success?");
        (boot_services.stall)(500_000);
        let r = (boot_services.get_memory_map)(
            &mut mem_map_size,
            mem_map,
            &mut map_key,
            &mut desc_size,
            &mut desc_ver,
        );
        if r != Status::SUCCESS {
            return r;
        }

        let r = (boot_services.exit_boot_services)(h, map_key);
        if r == Status::SUCCESS {
            break (mem_map, mem_map_size, desc_size);
        } else if r != Status::INVALID_PARAMETER {
            uefi_println!("Error: {r:?}");
            (boot_services.stall)(2_000_000);
            (boot_services.free_pool)(mem_map as *mut c_void);
            return r;
        }

        (boot_services.free_pool)(mem_map as *mut c_void);
        uefi_println!("Retrying...");
        (boot_services.stall)(2_000_000);
    };

    for y in 0..rh {
        for x in 0..rw {
            kernel::klib::put_pixel(
                fb_addr as *mut u8,
                pitch as usize,
                x as usize,
                y as usize,
                [0, 0, 0],
                pixel_format,
            );
        }
    }

    let r = kernel::kernel(
        mmap,
        mem_map_size,
        desc_size,
        fb_addr,
        pitch as usize,
        pixel_format,
    );

    (unsafe { &mut *(&mut *st).runtime_services }.reset_system)(r.0, r.1, 0, null_mut());

    Status::SUCCESS
}

//#[entry]
//fn efi_main() -> Status {
//    uefi::println!("Hello from UEFI boot service!");
//
//    let gop = boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
//    let mut gop = unsafe {
//        boot::open_protocol::<GraphicsOutput>(
//            OpenProtocolParams {
//                handle: gop,
//                agent: internal_image_handle,
//                controller: None,
//            },
//            OpenProtocolAttributes::GetProtocol,
//        )
//    }
//    .unwrap();
//
//    let mut fb = gop.frame_buffer();
//    let fb_ptr = fb.as_mut_ptr();
//    //let buf_len = fb.size();
//    let bytes_per_pixel = gop.current_mode_info().pixel_format();
//    let mode = gop.current_mode_info();
//    let pitch = mode.stride();
//    let (w, h) = mode.resolution();
//
//    uefi::println!("exiting...");
//    boot::stall(1_000_000);
//    uefi::println!("exit");
//    drop(gop);
//    // clear screen to black
//    for y in 0..h {
//        for x in 0..w {
//            kernel::klib::put_pixel(fb_ptr, pitch, x, y, [0, 0, 0], bytes_per_pixel);
//        }
//    }
//
//    let mmap = unsafe { boot::exit_boot_services(None) };
//
//    let r = kernel::kernel(
//        mmap,
//        unsafe { fb_ptr.as_mut().unwrap() },
//        pitch,
//        bytes_per_pixel,
//    );
//
//    runtime::reset(r.0, r.1, None)
//
//    //Status::SUCCESS
//}
