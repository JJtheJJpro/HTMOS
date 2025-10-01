//! **HyperText Markup Operating System Boot Information**
// HTMOSBI

// As for right now, I'm only going to have boot info support for UEFI.
// I will eventually have support for BIOS.

use core::ptr::null;

#[repr(C)]
pub struct HTMOSBootInformation {
    /// Physical memory map (array of entries)
    pub memory_map_addr: u64,
    pub memory_map_len: u64,
    pub memory_desc_size: u64,

    /// Framebuffer base address
    pub framebuffer_addr: u64,
    /// Width, height, pitch (stride in bytes), and pixel format
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_pitch: u32,
    pub framebuffer_bpp: u32, // bits per pixel

    /// Boot mode indicator (0 = BIOS, 1 = UEFI)
    pub boot_mode: u32,

    /// Reserved for BIOS, System Table pointer for UEFI (should be small enough to fit u32)
    pub reserved: u32,
}

static mut BOOT_INFO: *const HTMOSBootInformation = null();
pub fn boot_info() -> &'static HTMOSBootInformation {
    unsafe { &*BOOT_INFO }
}
pub fn boot_info_exists() -> bool {
    !unsafe { BOOT_INFO }.is_null()
}
pub(super) fn set_boot_info(v: *const HTMOSBootInformation) {
    if unsafe { BOOT_INFO }.is_null() {
        unsafe {
            BOOT_INFO = v;
        }
    } else {
        panic!("seriously?");
    }
}
