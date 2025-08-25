pub(crate) mod drivers;
pub(crate) mod klib;

extern crate alloc;

use crate::html;
use core::{
    fmt::{self, Arguments, Write},
    ptr::null_mut,
    sync::atomic::{AtomicPtr, Ordering},
};
use linked_list_allocator::LockedHeap;
use r_efi::{
    efi::{MemoryDescriptor, RESET_SHUTDOWN, ResetType, Status},
    protocols::graphics_output::GraphicsPixelFormat,
};
use typed_arena::Arena;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// The console that shows up right after boot services exit and will be dropped when the UI will be initialized.
struct KissConsole {
    frame_buf: u64,
    pitch: usize,
    pixel_format: GraphicsPixelFormat,
    x: usize,
    y: usize,
}

impl KissConsole {
    fn init(frame_buf: u64, pitch: usize, pixel_format: GraphicsPixelFormat) -> Self {
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
                    self.frame_buf as *mut u8,
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

pub(crate) static KISS_VAR: AtomicPtr<Kiss> = AtomicPtr::new(null_mut());

/// Kernel Info Status Structure
pub(crate) struct Kiss {
    mmap: *mut MemoryDescriptor,
    mem_map_size: usize,
    desc_size: usize,
    frame_buf: u64,
    console: Option<KissConsole>,
}

impl Kiss {
    /// <h1><i><u><b>CALL THIS FUNCTION FIRST OR NOTHING WILL WORK!!!</b></u></i></h1>
    fn init(
        mmap: *mut MemoryDescriptor,
        mem_map_size: usize,
        desc_size: usize,
        frame_buf: u64,
        pitch: usize,
        pixel_format: GraphicsPixelFormat,
    ) -> Self {
        Self {
            mmap,
            mem_map_size,
            desc_size,
            frame_buf,

            console: Some(KissConsole::init(frame_buf, pitch, pixel_format)),
        }
    }
}

/// INTERNAL API! Helper for print macros.
#[doc(hidden)]
pub(crate) fn print(args: Arguments) {
    unsafe { KISS_VAR.load(Ordering::Acquire).as_mut() }
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
        crate::print!("\n\n");
    };
    ($($arg:tt)*) => {
        crate::kernel::print(format_args!("{}{}", format_args!($($arg)*), "\n\n"));
    };
}

// I'm gonna worry about this later.
fn find_heap_region() -> Option<MemoryDescriptor> {
    const MIN_PAGES: usize = 65536; // this * 4096 = total bytes

    let kiss = unsafe { &mut *KISS_VAR.load(Ordering::Acquire) };

    println!("MEM MAP SIZE: {}", kiss.mem_map_size);
    println!("DESC SIZE: {}", kiss.desc_size);
    println!(
        "{}",
        if kiss.mem_map_size % kiss.desc_size == 0 {
            "TRUE"
        } else {
            "FALSE"
        }
    );

    let mut ptr = kiss.mmap as *const MemoryDescriptor;
    let count = kiss.mem_map_size / kiss.desc_size;
    for _ in 0..count {
        let desc = unsafe { &*ptr };
        if desc.r#type <= 10 {
            println!(
                "Type: {:?}, Start: {:#X}, Pages: {}",
                desc.r#type, desc.physical_start, desc.number_of_pages
            );
        }

        if (desc.r#type == 1
            || desc.r#type == 2
            || desc.r#type == 3
            || desc.r#type == 4
            || desc.r#type == 7)
            && desc.number_of_pages as usize > MIN_PAGES
        {
            return Some(*desc);
        }

        ptr = unsafe { (ptr as *const u8).add(kiss.desc_size) } as *const MemoryDescriptor;
    }

    None
}

pub fn pause() {
    loop {
        let sc = klib::read_ascii() as char;

        if sc == '\r' {
            break;
        }

        print!("{sc}");
    }
}

pub(crate) fn kernel(
    mmap: *mut MemoryDescriptor,
    mem_map_size: usize,
    desc_size: usize,
    frame_buf: u64,
    pitch: usize,
    pixel_format: GraphicsPixelFormat,
) -> (ResetType, Status) {
    let kiss = &mut Kiss::init(
        mmap,
        mem_map_size,
        desc_size,
        frame_buf,
        pitch,
        pixel_format,
    );
    KISS_VAR.store(kiss, Ordering::Release);
    if KISS_VAR.load(Ordering::Acquire).is_null() {
        unreachable!("KISS_VAR is null");
    }
    println!("TEST1");

    pause();

    println!("TEST2");

    let desc = match find_heap_region() {
        Some(v) => v,
        None => {
            println!("TESTFAIL");
            pause();
            return (RESET_SHUTDOWN, Status::ABORTED);
        }
    };

    println!("TEST3");
    klib::sleep_ms(1000);

    unsafe {
        ALLOCATOR.lock().init(
            desc.physical_start as *mut u8,
            desc.number_of_pages as usize,
        );
    }

    println!("HTMOS 0.0.0.1 - IN PRE-ALPHA STAGES (AKA WIP)");
    println!("PRESS ENTER TO SHUTDOWN...");

    let global_arena = Arena::new();

    let rawhtml = r#"
<html>
</html>
"#;
    let htmltree = html::parse(&global_arena, rawhtml).unwrap();

    //klib::read_line(&mut []);

    pause();
    pause();

    (RESET_SHUTDOWN, Status::SUCCESS)
}
