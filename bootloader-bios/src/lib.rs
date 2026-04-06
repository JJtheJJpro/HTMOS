#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    // Pointer to VGA buffer in Protected Mode
    const VGA_BUFFER: *mut u16 = 0xb8000 as *mut u16;

    // Write 'R' for Rust!
    unsafe {
        for b in *b"" {
            VGA_BUFFER.add();
        }
        VGA_BUFFER.write_volatile(b'R' as u16); // Character
        VGA_BUFFER.add(1).write_volatile(0x0B); // Color (Cyan)
    }

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
