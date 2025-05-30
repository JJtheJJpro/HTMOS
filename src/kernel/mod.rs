pub(crate) mod klib;

use core::{
    fmt::{self, Arguments, Write},
    panic::PanicInfo,
    ptr::null_mut,
    sync::atomic::{AtomicPtr, Ordering},
};
use uefi::{
    Status,
    mem::memory_map::MemoryMapOwned,
    proto::console::gop::PixelFormat,
    runtime::{self, ResetType},
};

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    crate::println!("[PANIC]: {info}");

    //klib::apic_stall(10_000_000, klib::calibrate_apic_hz());

    runtime::reset(
        ResetType::SHUTDOWN,
        Status::ABORTED,
        Some(info.message().as_str().unwrap_or_default().as_bytes()),
    )
}

/// The console that shows up right after boot services exit and will be dropped when the UI will be initialized.
struct KissConsole {
    frame_buf: *mut u8, // the reason why this is still kept as a raw pointer is to do raw pointer operations with ease.  We just initialize with a reference just to know that it's valid to use without panicking.
    pitch: usize,
    pixel_format: PixelFormat,
    x: usize,
    y: usize,
}

impl KissConsole {
    fn init(frame_buf: &'static mut u8, pitch: usize, pixel_format: PixelFormat) -> Self {
        Self {
            frame_buf,
            pitch,
            pixel_format,
            x: 0,
            y: 0,
        }
    }
    fn print_char(&mut self, c: u8) {
        match c {
            b'\r' => self.x = 0,
            b'\n' => {
                self.x = 0;
                self.y += 1;
            }
            8 => {
                self.x -= 1;
            }
            _ => {
                klib::draw_char(
                    self.frame_buf,
                    self.pitch,
                    self.x,
                    self.y,
                    c,
                    [0xFF, 0xFF, 0xFF],
                    [0, 0, 0],
                    self.pixel_format,
                );
                self.x += 1;
            }
        }
    }
}

impl Write for KissConsole {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        //klib::draw_string(
        //    self.frame_buf,
        //    self.pitch,
        //    self.x,
        //    self.y,
        //    s,
        //    [0xFF, 0xFF, 0xFF],
        //    [0, 0, 0],
        //    self.pixel_format,
        //);

        for &c in s.as_bytes() {
            self.print_char(c);
        }

        Ok(())
    }
}

static KISS_VAR: AtomicPtr<Kiss> = AtomicPtr::new(null_mut());

/// Kernel Info Status Structure
struct Kiss {
    mmap: MemoryMapOwned,
    frame_buf: *mut u8,

    console: Option<KissConsole>,
}

impl Kiss {
    /// <h1><i><u><b>CALL THIS FUNCTION FIRST OR NOTHING WILL WORK!!!</b></u></i></h1>
    fn init(
        mmap: MemoryMapOwned,
        frame_buf: &'static mut u8,
        pitch: usize,
        pixel_format: PixelFormat,
    ) {
        KISS_VAR.store(
            &mut Self {
                mmap,
                frame_buf,

                console: Some(KissConsole::init(frame_buf, pitch, pixel_format)),
            },
            Ordering::SeqCst,
        );
    }
}

/// INTERNAL API! Helper for print macros.
#[doc(hidden)]
fn print(args: Arguments) {
    unsafe { KISS_VAR.load(Ordering::SeqCst).as_mut() }
        .unwrap()
        .console
        .as_mut()
        .unwrap()
        .write_fmt(args)
        .unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        crate::kernel::print(format_args!($($arg)*));
    };
}
#[macro_export]
macro_rules! println {
    () => {
        crate::print!("\n");
    };
    ($($arg:tt)*) => {
        crate::kernel::print(format_args!("{}{}", format_args!($($arg)*), "\n"));
    };
}

pub(crate) fn kernel(
    mmap: MemoryMapOwned,
    frame_buf: &'static mut u8,
    pitch: usize,
    pixel_format: PixelFormat,
) -> (ResetType, Status) {
    Kiss::init(mmap, frame_buf, pitch, pixel_format);
    if KISS_VAR.load(Ordering::SeqCst).is_null() {
        unreachable!("KISS_VAR is null");
    }

    println!("JJOS 0.0.0.1 - IN PRE-ALPHA STAGES (AKA WIP)");
    println!("PRESS ENTER TO SHUTDOWN...");

    //klib::read_line(&mut []);

    loop {
        let sc = klib::read_ascii() as char;

        if sc == '\r' {
            break;
        }

        print!("{sc}");
    }

    (ResetType::SHUTDOWN, Status::SUCCESS)
}
