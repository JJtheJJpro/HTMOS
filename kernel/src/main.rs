#![no_std]
#![no_main]

mod api;
mod boot_info;
mod htmalloc;
mod kiss;

use crate::{boot_info::HTMOSBootInformation, htmalloc::HTMAlloc, kiss::KissConsole};

#[global_allocator]
static HTMAS: HTMAlloc = HTMAlloc::ginit();

#[unsafe(no_mangle)]
extern "C" fn htmkrnl(info: *const HTMOSBootInformation) -> ! {
    if info.is_null() {
        panic!("no boot info given");
    }
    //(unsafe { &mut *(&mut *((&*info).reserved as *mut SystemTable)).runtime_services }
    //    .reset_system)(RESET_COLD, Status::ABORTED, 0, null_mut());
    boot_info::set_boot_info(info);
    HTMAS.update();

    kiss::fill_screen(0, 0xFF, 0);
    kiss::fill_screen(0, 0, 0);

    let mut kc = KissConsole::new();
    //kc.print_ascii_str("!\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~");
    kc.print_ascii_str("HTMOS Pre-Alpha v0.1");

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
