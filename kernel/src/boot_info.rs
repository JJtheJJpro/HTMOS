//! **HyperText Markup Operating System Boot Information**
// HTMOSBI

// As for right now, I'm only going to have boot info support for UEFI.
// I will eventually have support for BIOS.

use core::ptr::null;
use htmos_boot_info::HTMOSBootInformation;

static mut BOOT_INFO: *const HTMOSBootInformation = null();
pub fn boot_info() -> &'static HTMOSBootInformation {
    unsafe { &*BOOT_INFO }
}
//pub fn boot_info_exists() -> bool {
//    !unsafe { BOOT_INFO }.is_null()
//}
pub(super) fn set_boot_info(v: *const HTMOSBootInformation) {
    // Same idea as std::sync::Once.
    if unsafe { BOOT_INFO }.is_null() {
        unsafe {
            BOOT_INFO = v;
        }
    } else {
        panic!("seriously?");
    }
}
