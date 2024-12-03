// Copyright 2024 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::strip::Tile;

const TILE_WIDTH: u32 = 4;
const TILE_HEIGHT: u32 = 4;

const TILE_SCALE_X: f32 = 1.0 / TILE_WIDTH as f32;
const TILE_SCALE_Y: f32 = 1.0 / TILE_HEIGHT as f32;

/// This is just Line but f32
#[derive(Clone, Copy)]
pub struct FlatLine {
    // should these be vec2?
    pub p0: [f32; 2],
    pub p1: [f32; 2],
}

impl FlatLine {
    pub fn new(p0: [f32; 2], p1: [f32; 2]) -> Self {
        Self { p0, p1 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    fn from_array(xy: [f32; 2]) -> Self {
        Vec2::new(xy[0], xy[1])
    }

    // Note: this assumes values in range.
    fn pack(self) -> u32 {
        // TODO: scale should depend on tile size
        let x = (self.x * 8192.0).round() as u32;
        let y = (self.y * 8192.0).round() as u32;
        (y << 16) + x
    }

    pub fn unpack(packed: u32) -> Self {
        let x = (packed & 0xffff) as f32 * (1.0 / 8192.0);
        let y = (packed >> 16) as f32 * (1.0 / 8192.0);
        Vec2::new(x, y)
    }
}

impl std::ops::Add for Vec2 {
    type Output = Self;

    fn add(self, rhs: Vec2) -> Self {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Self;

    fn sub(self, rhs: Vec2) -> Self {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl std::ops::Mul<f32> for Vec2 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Vec2::new(self.x * rhs, self.y * rhs)
    }
}

fn span(a: f32, b: f32) -> u32 {
    (a.max(b).ceil() - a.min(b).floor()).max(1.0) as u32
}

const ONE_MINUS_ULP: f32 = 0.99999994;

pub(crate) const ROBUST_EPSILON: f32 = 2e-7;

pub fn make_tiles(lines: &[FlatLine], tile_buf: &mut Vec<Tile>) {
    tile_buf.clear();
    for line in lines {
        let p0 = Vec2::from_array(line.p0);
        let p1 = Vec2::from_array(line.p1);
        let is_down = p1.y >= p0.y;
        let (orig_xy0, orig_xy1) = if is_down { (p0, p1) } else { (p1, p0) };
        let s0 = orig_xy0 * TILE_SCALE_X;
        let s1 = orig_xy1 * TILE_SCALE_Y;
        let count_x = span(s0.x, s1.x) - 1;
        let count = count_x + span(s0.y, s1.y);

        let dx = (s1.x - s0.x).abs();
        let dy = s1.y - s0.y;
        if dx + dy == 0.0 {
            continue;
        }
        if dy == 0.0 && s0.y.floor() == s0.y {
            continue;
        }
        let idxdy = 1.0 / (dx + dy);
        let mut a = dx * idxdy;
        let is_positive_slope = s1.x >= s0.x;
        let sign = if is_positive_slope { 1.0 } else { -1.0 };
        let xt0 = (s0.x * sign).floor();
        let c = s0.x * sign - xt0;
        let y0 = s0.y.floor();
        let ytop = if s0.y == s1.y { s0.y.ceil() } else { y0 + 1.0 };
        let b = ((dy * c + dx * (ytop - s0.y)) * idxdy).min(ONE_MINUS_ULP);
        let robust_err = (a * (count as f32 - 1.0) + b).floor() - count_x as f32;
        if robust_err != 0.0 {
            a -= ROBUST_EPSILON.copysign(robust_err);
        }
        let x0 = xt0 * sign + if is_positive_slope { 0.0 } else { -1.0 };

        let imin = 0;
        let imax = count;
        // In the Vello source, here's where we do clipping to viewport (by setting
        // imin and imax to more restrictive values).
        // Note: we don't really need to compute this if imin == 0, but it's cheap
        let mut last_z = (a * (imin as f32 - 1.0) + b).floor();
        for i in imin..imax {
            let zf = a * i as f32 + b;
            let z = zf.floor();
            let y = (y0 + i as f32 - z) as i32;
            let x = (x0 + sign * z) as i32;

            let tile_xy = Vec2::new(x as f32 * TILE_WIDTH as f32, y as f32 * TILE_HEIGHT as f32);
            let tile_xy1 = tile_xy + Vec2::new(TILE_WIDTH as f32, TILE_HEIGHT as f32);

            let mut xy0 = orig_xy0;
            let mut xy1 = orig_xy1;
            if i > 0 {
                if z == last_z {
                    // Top edge is clipped
                    // This calculation should arguably be done on orig_xy. Also might
                    // be worth retaining slope.
                    let mut xt = xy0.x + (xy1.x - xy0.x) * (tile_xy.y - xy0.y) / (xy1.y - xy0.y);
                    xt = xt.clamp(tile_xy.x + 1e-3, tile_xy1.x);
                    xy0 = Vec2::new(xt, tile_xy.y);
                } else {
                    // If is_positive_slope, left edge is clipped, otherwise right
                    let x_clip = if is_positive_slope {
                        tile_xy.x
                    } else {
                        tile_xy1.x
                    };
                    let mut yt = xy0.y + (xy1.y - xy0.y) * (x_clip - xy0.x) / (xy1.x - xy0.x);
                    yt = yt.clamp(tile_xy.y + 1e-3, tile_xy1.y);
                    xy0 = Vec2::new(x_clip, yt);
                }
            }
            if i < count - 1 {
                let z_next = (a * (i as f32 + 1.0) + b).floor();
                if z == z_next {
                    // Bottom edge is clipped
                    let mut xt = xy0.x + (xy1.x - xy0.x) * (tile_xy1.y - xy0.y) / (xy1.y - xy0.y);
                    xt = xt.clamp(tile_xy.x + 1e-3, tile_xy1.x);
                    xy1 = Vec2::new(xt, tile_xy1.y);
                } else {
                    // If is_positive_slope, right edge is clipped, otherwise left
                    let x_clip = if is_positive_slope {
                        tile_xy1.x
                    } else {
                        tile_xy.x
                    };
                    let mut yt = xy0.y + (xy1.y - xy0.y) * (x_clip - xy0.x) / (xy1.x - xy0.x);
                    yt = yt.clamp(tile_xy.y + 1e-3, tile_xy1.y);
                    xy1 = Vec2::new(x_clip, yt);
                }
            }
            // Apply numerical robustness logic
            let mut p0 = xy0 - tile_xy;
            let mut p1 = xy1 - tile_xy;
            // one count in fixed point
            const EPSILON: f32 = 1.0 / 8192.0;
            if p0.x < EPSILON {
                if p1.x < EPSILON {
                    p0.x = EPSILON;
                    if p0.y < EPSILON {
                        // Entire tile
                        p1.x = EPSILON;
                        p1.y = TILE_HEIGHT as f32;
                    } else {
                        // Make segment disappear
                        p1.x = 2.0 * EPSILON;
                        p1.y = p0.y;
                    }
                } else if p0.y < EPSILON {
                    p0.x = EPSILON;
                }
            } else if p1.x < EPSILON {
                if p1.y < EPSILON {
                    p1.x = EPSILON;
                }
            }
            // Question: do we need these? Also, maybe should be post-rounding?
            if p0.x == p0.x.floor() && p0.x != 0.0 {
                p0.x -= EPSILON;
            }
            if p1.x == p1.x.floor() && p1.x != 0.0 {
                p1.x -= EPSILON;
            }
            if !is_down {
                (p0, p1) = (p1, p0);
            }
            // These are regular asserts in Vello, but are debug asserts
            // here for performance reasons.
            debug_assert!(p0.x >= 0.0 && p0.x <= TILE_WIDTH as f32);
            debug_assert!(p0.y >= 0.0 && p0.y <= TILE_HEIGHT as f32);
            debug_assert!(p1.x >= 0.0 && p1.x <= TILE_WIDTH as f32);
            debug_assert!(p1.y >= 0.0 && p1.y <= TILE_HEIGHT as f32);
            let tile = Tile {
                x: x as u16,
                y: y as u16,
                p0: p0.pack(),
                p1: p1.pack(),
            };
            tile_buf.push(tile);

            last_z = z;
        }
    }
    // This particular choice of sentinel tiles generates a sentinel strip.
    tile_buf.push(Tile {
        x: 0x3ffd,
        y: 0x3fff,
        p0: 0,
        p1: 0,
    });
    tile_buf.push(Tile {
        x: 0x3fff,
        y: 0x3fff,
        p0: 0,
        p1: 0,
    });
}
