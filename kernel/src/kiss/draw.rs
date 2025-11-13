//! HTMOS's Framebuffer Drawing Driver

use crate::kiss::{self, RGB};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

pub fn draw_line(x0: i32, y0: i32, x1: i32, y1: i32, color: RGB) {
    let mut x0 = x0;
    let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        kiss::set_pixel(x0 as u32, y0 as u32, color).unwrap();
        if x0 == x1 && y0 == y1 {
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
    let step = 0.5; // degrees per pixel step - smaller = smoother
    let mut angle = start_deg;

    while angle <= end_deg {
        let rad = angle * core::f32::consts::PI / 180.0;
        let x = cx + (radius as f32 * libm::cosf(rad)) as i32;
        let y = cy + (radius as f32 * libm::sinf(rad)) as i32;
        kiss::set_pixel(x as u32, y as u32, color).unwrap();
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

        kiss::set_pixel((cx + x as i32) as u32, (cy + y as i32) as u32, color).unwrap();

        angle += step;
    }
}

pub fn draw_rect(rect: Rect, color: RGB) {
    draw_line(rect.x, rect.y, rect.x, rect.y + rect.h as i32, color);
    draw_line(
        rect.x,
        rect.y + rect.h as i32,
        rect.x + rect.w as i32,
        rect.y + rect.h as i32,
        color,
    );
    draw_line(
        rect.x + rect.w as i32,
        rect.y + rect.h as i32,
        rect.x + rect.w as i32,
        rect.y,
        color,
    );
    draw_line(
        rect.x + rect.w as i32,
        rect.y,
        rect.x,
        rect.y + rect.h as i32,
        color,
    );
}
