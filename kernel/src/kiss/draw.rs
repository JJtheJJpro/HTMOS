//! HTMOS's Framebuffer Drawing Driver

use crate::{boot_info::boot_info, kiss::RGB, print, println};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
impl Point {
    pub const fn left(&self, n: i32) -> Point {
        Point {
            x: self.x - n,
            y: self.y,
        }
    }
    pub const fn up(&self, n: i32) -> Point {
        Point {
            x: self.x,
            y: self.y - n,
        }
    }
    pub const fn right(&self, n: i32) -> Point {
        Point {
            x: self.x + n,
            y: self.y,
        }
    }
    pub const fn down(&self, n: i32) -> Point {
        Point {
            x: self.x,
            y: self.y + n,
        }
    }
}
impl PartialEq for Point {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}
impl Rect {
    pub const fn from_ltrb(l: i32, t: i32, r: i32, b: i32) -> Self {
        Self {
            x: if l > r { r } else { l },
            y: if t > b { b } else { t },
            w: (r - l).unsigned_abs(),
            h: (b - t).unsigned_abs(),
        }
    }

    pub const fn ltrb(&self) -> (i32, i32, i32, i32) {
        (
            self.x,
            self.y,
            self.x + self.w.cast_signed(),
            self.y + self.h.cast_signed(),
        )
    }

    pub const fn tl(&self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }
    pub const fn tr(&self) -> Point {
        Point {
            x: self.x + self.w.cast_signed(),
            y: self.y,
        }
    }
    pub const fn bl(&self) -> Point {
        Point {
            x: self.x,
            y: self.y + self.h.cast_signed(),
        }
    }
    pub const fn br(&self) -> Point {
        Point {
            x: self.x + self.w.cast_signed(),
            y: self.y + self.h.cast_signed(),
        }
    }
}

pub fn draw_horizontal_line(spt: Point, w: u32, width: u32, color: RGB) {
    let bi = boot_info();

    for x in 0..w {
        crate::pixel!(
            bi.framebuffer_format,
            bi.framebuffer_addr,
            bi.framebuffer_pitch,
            spt.x.cast_unsigned() + x,
            spt.y.cast_unsigned(),
            color
        );
    }
}

pub fn draw_vertical_line(spt: Point, h: u32, width: u32, color: RGB) {
    let bi = boot_info();

    for y in 0..h {
        crate::pixel!(
            bi.framebuffer_format,
            bi.framebuffer_addr,
            bi.framebuffer_pitch,
            spt.x.cast_unsigned(),
            spt.y.cast_unsigned() + y,
            color
        );
    }
}

pub fn draw_line(pt1: Point, pt2: Point, width: u32, color: RGB) {
    if pt1 == pt2 {
        let bi = boot_info();
        crate::pixel!(
            bi.framebuffer_format,
            bi.memory_map_addr,
            bi.framebuffer_pitch,
            pt1.x.cast_unsigned(),
            pt2.y.cast_unsigned(),
            color
        );
        return;
    } else if pt1.x == pt2.x {
        draw_vertical_line(
            if pt1.y < pt2.y { pt1 } else { pt2 },
            (pt2.y - pt1.y).unsigned_abs(),
            width,
            color,
        );
        return;
    } else if pt1.y == pt2.y {
        draw_horizontal_line(
            if pt1.x < pt2.x { pt1 } else { pt2 },
            (pt2.x - pt1.x).unsigned_abs(),
            width,
            color,
        );
        return;
    }

    let mut x0 = pt1.x;
    let mut y0 = pt1.y;
    let dx = (pt2.x as i32 - x0 as i32).abs();
    let sx = if x0 < pt2.x { 1 } else { -1 };
    let dy = -(pt2.y as i32 - y0 as i32).abs();
    let sy = if y0 < pt2.y { 1 } else { -1 };
    let mut err = dx + dy;

    let bi = boot_info();
    loop {
        crate::pixel!(
            bi.framebuffer_format,
            bi.framebuffer_addr,
            bi.framebuffer_pitch,
            x0.cast_unsigned(),
            y0.cast_unsigned(),
            color
        );

        if x0 == pt2.x && y0 == pt2.y {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

pub fn draw_arc(cx: i32, cy: i32, radius: i32, start_deg: f32, end_deg: f32, color: RGB) {
    let bi = boot_info();

    let step = 0.5; // degrees per pixel step - smaller = smoother
    let mut angle = start_deg;

    while angle <= end_deg {
        let rad = angle * core::f32::consts::PI / 180.0;
        let x = cx + (radius as f32 * libm::cosf(rad)) as i32;
        let y = cy - (radius as f32 * libm::sinf(rad)) as i32;
        crate::pixel!(
            bi.framebuffer_format,
            bi.framebuffer_addr,
            bi.framebuffer_pitch,
            x as u32,
            y as u32,
            color
        );
        angle += step;
    }
}

pub fn draw_ellipse_rotated(
    cx: i32,
    cy: i32,
    width: f32,
    height: f32,
    rotation_deg: f32,
    color: RGB,
) {
    let bi = boot_info();

    let step = 0.5; // degrees per sample
    let rx = width / 2.0;
    let ry = height / 2.0;
    let rot = rotation_deg * core::f32::consts::PI / 180.0;

    let cos_rot = libm::cosf(rot);
    let sin_rot = libm::sinf(rot);

    let mut angle = 0.0;
    while angle < 360.0 {
        let rad = angle * core::f32::consts::PI / 180.0;

        // Parametric ellipse (before rotation)
        let x_unrot = rx * libm::cosf(rad);
        let y_unrot = ry * libm::sinf(rad);

        // Apply rotation matrix:
        // [x'] = [cos -sin][x]
        // [y']   [sin  cos][y]
        let x = x_unrot * cos_rot - y_unrot * sin_rot;
        let y = x_unrot * sin_rot + y_unrot * cos_rot;

        crate::pixel!(
            bi.framebuffer_format,
            bi.framebuffer_addr,
            bi.framebuffer_pitch,
            (cx + x as i32) as u32,
            (cy + y as i32) as u32,
            color
        );

        angle += step;
    }
}

pub fn draw_rect(rect: Rect, width: u32, color: RGB) {
    draw_line(rect.tl(), rect.bl(), width, color);
    draw_line(rect.bl(), rect.br(), width, color);
    draw_line(rect.br(), rect.tr(), width, color);
    draw_line(rect.tr(), rect.tl(), width, color);
}

pub fn draw_rounded_rect(rect: Rect, radius: i32, width: u32, color: RGB) {
    let (l, t, r, b) = rect.ltrb();
    draw_arc(l + radius, t + radius, radius, 90.0, 180.0, color);
    draw_line(rect.tl().down(radius), rect.bl().up(radius), width, color);
    draw_arc(l + radius, b - radius, radius, 180.0, 270.0, color);
    draw_line(
        rect.bl().right(radius),
        rect.br().left(radius),
        width,
        color,
    );
    draw_arc(r - radius, b - radius, radius, 270.0, 360.0, color);
    draw_line(rect.br().up(radius), rect.tr().down(radius), width, color);
    draw_arc(r - radius, t + radius, radius, 0.0, 90.0, color);
    draw_line(
        rect.tr().left(radius),
        rect.tl().right(radius),
        width,
        color,
    );
}
