use alloc::vec::Vec;
use uefi::{
    boot::{self, OpenProtocolAttributes, OpenProtocolParams},
    proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput},
};

pub struct Buffer {
    width: usize,
    height: usize,
    pixels: Vec<BltPixel>,
}

impl Buffer {
    /// Create a new `Buffer`.
    pub fn new(width: usize, height: usize) -> Self {
        Buffer {
            width,
            height,
            pixels: alloc::vec![BltPixel::new(0, 0, 0); width * height],
        }
    }

    /// Create a new `Buffer` with the current resolution config.
    pub fn current() -> Self {
        let (w, h) = get_resolution();
        Self::new(w, h)
    }

    /// Get a single pixel.
    fn pixel(&mut self, x: usize, y: usize) -> Option<&mut BltPixel> {
        self.pixels.get_mut(y * self.width + x)
    }

    /// Blit the buffer to the framebuffer.
    fn blit(&self, gop: &mut GraphicsOutput) -> Result<(), uefi::Error> {
        gop.blt(BltOp::BufferToVideo {
            buffer: &self.pixels,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (self.width, self.height),
        })
    }

    /// Update only a pixel to the framebuffer.
    fn blit_pixel(
        &self,
        gop: &mut GraphicsOutput,
        coords: (usize, usize),
    ) -> Result<(), uefi::Error> {
        gop.blt(BltOp::BufferToVideo {
            buffer: &self.pixels,
            src: BltRegion::SubRectangle {
                coords,
                px_stride: self.width,
            },
            dest: coords,
            dims: (1, 1),
        })
    }

    pub fn rect(
        &mut self,
        gop: &mut GraphicsOutput,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        rgb: (u8, u8, u8),
    ) -> Result<(), uefi::Error> {
        for p in self.pixels.as_mut_slice() {
            p.red = rgb.0;
            p.green = rgb.1;
            p.blue = rgb.2;
        }
        gop.blt(BltOp::BufferToVideo {
            buffer: &self.pixels,
            src: BltRegion::SubRectangle {
                coords: (x, y),
                px_stride: w,
            },
            dest: (x, y),
            dims: (w, h),
        })
    }
}

#[derive(Clone, Copy)]
pub struct Point {
    x: f32,
    y: f32,
}

impl Point {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

pub fn get_resolution() -> (usize, usize) {
    let gop_handle = boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
    let gop = unsafe {
        boot::open_protocol::<GraphicsOutput>(
            OpenProtocolParams {
                handle: gop_handle,
                agent: boot::image_handle(),
                controller: None,
            },
            OpenProtocolAttributes::GetProtocol,
        )
        .unwrap()
    };
    gop.current_mode_info().resolution()
}
