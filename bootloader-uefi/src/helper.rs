use core::{
    ffi::c_void,
    fmt::{Arguments, Write},
    panic::PanicInfo,
    ptr::null_mut,
    sync::atomic::{AtomicPtr, Ordering},
};
use r_efi::{
    efi::{self, MemoryDescriptor, Status, SystemTable},
    protocols,
};

/// The Panic Handler (needs to be updated for just UEFI use).
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    if boot_services_alive() {
        crate::uefi_println!("[PANIC]: {info}");
        (unsafe { &mut *(&mut *SYS_TBL.load(Ordering::Acquire)).boot_services }.stall)(5_000_000);
    } else {
        (unsafe { &mut *(&mut *SYS_TBL.load(Ordering::Acquire)).runtime_services }.reset_system)(
            efi::RESET_SHUTDOWN,
            Status::SUCCESS,
            0,
            null_mut(),
        );
    }

    (unsafe { &mut *(&mut *SYS_TBL.load(Ordering::Acquire)).runtime_services }.reset_system)(
        efi::RESET_SHUTDOWN,
        Status::ABORTED,
        0,
        null_mut(),
    );

    loop {}
}

/// Prints to the UEFI output.
#[macro_export]
macro_rules! uefi_print {
    ($($arg:tt)*) => {
        crate::helper::print(format_args!($($arg)*));
    };
}
/// Prints to the UEFI output with a new line.
#[macro_export]
macro_rules! uefi_println {
    () => {
        uefi_print!("\r\n");
    };
    ($($arg:tt)*) => {
        crate::helper::print(format_args!("{}{}", format_args!($($arg)*), "\r\n"));
    };
}
/// Converts a given Rust string to a C-16 UEFI string.
#[macro_export]
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

/// The System Table (used static for access anywhere).
pub(crate) static SYS_TBL: AtomicPtr<SystemTable> = AtomicPtr::new(null_mut());
pub(crate) static HANDLE: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

/// A placeholder and helper for printing to the UEFI output using arguments and formatters.
struct WriteHolder;
impl Write for WriteHolder {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let con_out = unsafe { &mut *(&mut *SYS_TBL.load(Ordering::Acquire)).con_out };
        (con_out.output_string)(con_out, crate::cstr16!(s));
        Ok(())
    }
}

/// A print helper function for printing to the UEFI output.
#[doc(hidden)]
pub(crate) fn print(args: Arguments) {
    WriteHolder {}.write_fmt(args).unwrap();
}

/// Returns true if boot services have not been exited; otherwise, false.
pub(crate) fn boot_services_alive() -> bool {
    unsafe { &mut *SYS_TBL.load(Ordering::Acquire) }.boot_services != null_mut()
}

/// Exits boot services, returning valid MemoryDescriptor buffer pointer and respective sizes regarding it.
pub(crate) fn exit_boot_services() -> Result<(*mut MemoryDescriptor, usize, usize), Status> {
    let boot_services = unsafe { &mut *(&mut *SYS_TBL.load(Ordering::Acquire)).boot_services };
    let h = HANDLE.load(Ordering::Acquire);

    loop {
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
            return Err(r);
        }
        mem_map_size += desc_size * 2;
        let r = (boot_services.allocate_pool)(
            efi::LOADER_DATA,
            mem_map_size,
            &mut mem_map as *mut _ as *mut *mut c_void,
        );
        if r != Status::SUCCESS {
            return Err(r);
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
            return Err(r);
        }

        let r = (boot_services.exit_boot_services)(h, map_key);
        if r == Status::SUCCESS {
            return Ok((mem_map, mem_map_size, desc_size));
        } else if r != Status::INVALID_PARAMETER {
            uefi_println!("Error: {r:?}");
            (boot_services.stall)(2_000_000);
            (boot_services.free_pool)(mem_map as *mut c_void);
            return Err(r);
        }

        (boot_services.free_pool)(mem_map as *mut c_void);
        uefi_println!("Retrying...");
        (boot_services.stall)(2_000_000);
    }
}

pub(crate) fn load_file(
    mut directory: *mut protocols::file::Protocol,
    path: *mut u16,
) -> *mut protocols::file::Protocol {
    let h = HANDLE.load(Ordering::Acquire);
    let st = unsafe { &mut *SYS_TBL.load(Ordering::Acquire) };
    let bs = unsafe { &mut *st.boot_services };

    let mut loaded_file = null_mut();

    let mut loaded_image: *mut protocols::loaded_image::Protocol = null_mut();
    let r = (bs.open_protocol)(
        h,
        &protocols::loaded_image::PROTOCOL_GUID as *const _ as *mut _,
        &mut loaded_image as *mut _ as *mut _,
        h,
        null_mut(),
        efi::OPEN_PROTOCOL_BY_HANDLE_PROTOCOL,
    );
    if r != Status::SUCCESS {
        panic!("nope1");
    }

    let mut file_system: *mut protocols::simple_file_system::Protocol = null_mut();
    let r = (bs.open_protocol)(
        unsafe { &*loaded_image }.device_handle,
        &protocols::simple_file_system::PROTOCOL_GUID as *const _ as *mut _,
        &mut file_system as *mut _ as *mut _,
        h,
        null_mut(),
        efi::OPEN_PROTOCOL_BY_HANDLE_PROTOCOL,
    );
    if r != Status::SUCCESS {
        panic!("nope2");
    }

    if directory.is_null() {
        let r = (unsafe { &mut *file_system }.open_volume)(file_system, &mut directory);
        if r != Status::SUCCESS {
            panic!("nope3");
        }
    }

    let r = (unsafe { &mut *directory }.open)(
        directory,
        &mut loaded_file,
        path,
        efi::protocols::file::MODE_READ,
        efi::protocols::file::READ_ONLY,
    );
    if r != Status::SUCCESS {
        panic!("nope4");
    }

    return loaded_file;
}
