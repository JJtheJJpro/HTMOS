#![no_std]
#![no_main]

mod kernel;

use uefi::{
    boot::{OpenProtocolAttributes, OpenProtocolParams},
    prelude::*,
    proto::console::gop::GraphicsOutput,
};

#[entry]
fn efi_main() -> Status {
    uefi::println!("Hello from UEFI boot service!");

    let gop = boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
    let mut gop = unsafe {
        boot::open_protocol::<GraphicsOutput>(
            OpenProtocolParams {
                handle: gop,
                agent: internal_image_handle,
                controller: None,
            },
            OpenProtocolAttributes::GetProtocol,
        )
    }
    .unwrap();

    let mut fb = gop.frame_buffer();
    let fb_ptr = fb.as_mut_ptr();
    //let buf_len = fb.size();
    let bytes_per_pixel = gop.current_mode_info().pixel_format();
    let mode = gop.current_mode_info();
    let pitch = mode.stride();
    let (w, h) = mode.resolution();

    uefi::println!("exiting...");
    boot::stall(1_000_000);
    uefi::println!("exit");
    drop(gop);
    // clear screen to black
    for y in 0..h {
        for x in 0..w {
            kernel::klib::put_pixel(fb_ptr, pitch, x, y, [0, 0, 0], bytes_per_pixel);
        }
    }

    let mmap = unsafe { boot::exit_boot_services(None) };

    let r = kernel::kernel(
        mmap,
        unsafe { fb_ptr.as_mut().unwrap() },
        pitch,
        bytes_per_pixel,
    );

    runtime::reset(r.0, r.1, None)

    //Status::SUCCESS
}
