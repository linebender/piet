// Copyright 2024 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// CPU implementation of sparse strip rendering

use piet_next::peniko::color::PremulRgba8;

use crate::{
    flatten::SoupBowl,
    tiling::{make_tiles, Vec2},
};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Loc {
    path_id: u32,
    x: u16,
    y: u16,
}

struct Footprint(u32);

pub struct Tile {
    // TODO: use loc inline?
    pub path_id: u32,
    pub x: u16,
    pub y: u16,
    pub p0: u32, // packed
    pub p1: u32, // packed
}

impl std::fmt::Debug for Tile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let p0 = Vec2::unpack(self.p0);
        let p1 = Vec2::unpack(self.p1);
        write!(
            f,
            "Tile {{ path_id: {}, xy: ({}, {}), p0: ({:.4}, {:.4}), p1: ({:.4}, {:.4}) }}",
            self.path_id, self.x, self.y, p0.x, p0.y, p1.x, p1.y
        )
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Strip {
    pub path_id: u32,
    pub xy: u32, // this could be u16's on the Rust side
    pub col: u32,
    pub winding: i32,
}

impl Loc {
    fn same_strip(&self, other: &Self) -> bool {
        self.same_row(other) && (other.x - self.x) / 2 == 0
    }

    fn same_row(&self, other: &Self) -> bool {
        self.path_id == other.path_id && self.y == other.y
    }
}

impl Tile {
    #[allow(unused)]
    /// Create a tile from synthetic data.
    fn new(loc: Loc, footprint: Footprint, delta: i32) -> Self {
        let p0 = (delta == -1) as u32 * 65536 + footprint.0.trailing_zeros() * 8192;
        let p1 = (delta == 1) as u32 * 65536 + (32 - footprint.0.leading_zeros()) * 8192;
        Tile {
            path_id: loc.path_id,
            x: loc.x,
            y: loc.y,
            p0,
            p1,
        }
    }

    fn loc(&self) -> Loc {
        Loc {
            path_id: self.path_id,
            x: self.x,
            y: self.y,
        }
    }

    fn footprint(&self) -> Footprint {
        let x0 = (self.p0 & 0xffff) as f32 * (1.0 / 8192.0);
        let x1 = (self.p1 & 0xffff) as f32 * (1.0 / 8192.0);
        // On CPU, might be better to do this as fixed point
        let xmin = x0.min(x1).floor() as u32;
        let xmax = (xmin + 1).max(x0.max(x1).ceil() as u32);
        Footprint((1 << xmax) - (1 << xmin))
    }

    fn delta(&self) -> i32 {
        ((self.p1 >> 16) == 0) as i32 - ((self.p0 >> 16) == 0) as i32
    }

    // Comparison function for sorting. Only compares loc, doesn't care
    // about points. Unpacking code has been validated to be efficient in
    // Godbolt.
    pub fn cmp(&self, b: &Tile) -> std::cmp::Ordering {
        let xya = ((self.y as u32) << 16) + (self.x as u32);
        let xyb = ((b.y as u32) << 16) + (b.x as u32);
        (self.path_id, xya).cmp(&(b.path_id, xyb))
    }
}

fn render_strips(tiles: &[Tile]) -> (Vec<Strip>, Vec<u32>) {
    let mut strips = vec![];
    let mut out = vec![];
    let mut strip_start = true;
    let mut cols = 0;
    let mut prev_tile = &tiles[0];
    let mut fp = prev_tile.footprint().0;
    let mut seg_start = 0;
    let mut delta = 0;
    // Note: add a sentinel tile in input
    for i in 1..tiles.len() {
        let tile = &tiles[i];
        //println!("{tile:?}");
        if prev_tile.loc() != tile.loc() {
            let start_delta = delta;
            let same_strip = prev_tile.loc().same_strip(&tile.loc());
            if same_strip {
                fp |= 8;
            }
            let x0 = fp.trailing_zeros();
            let x1 = 32 - fp.leading_zeros();
            for tile in &tiles[seg_start..i] {
                delta += tile.delta();
            }
            // (x0..x1) is range of columns we need to render
            for x in x0..x1 {
                let mut areas = [start_delta as f32; 4];
                // probably want to reorder loops for efficiency
                for tile in &tiles[seg_start..i] {
                    let p0 = Vec2::unpack(tile.p0);
                    let p1 = Vec2::unpack(tile.p1);
                    let slope = (p1.x - p0.x) / (p1.y - p0.y);
                    let startx = p0.x - x as f32;
                    for y in 0..4 {
                        let starty = p0.y - y as f32;
                        let y0 = starty.clamp(0.0, 1.0);
                        let y1 = (p1.y - y as f32).clamp(0.0, 1.0);
                        let dy = y0 - y1;
                        // Note: getting rid of this predicate might help with
                        // auto-vectorization.
                        if dy != 0.0 {
                            let xx0 = startx + (y0 - starty) * slope;
                            let xx1 = startx + (y1 - starty) * slope;
                            let xmin0 = xx0.min(xx1);
                            let xmax = xx0.max(xx1);
                            let xmin = xmin0.min(1.0) - 1e-6;
                            let b = xmax.min(1.0);
                            let c = b.max(0.0);
                            let d = xmin.max(0.0);
                            let a = (b + 0.5 * (d * d - c * c) - xmin) / (xmax - xmin);
                            areas[y] += a * dy;
                        }
                        if p0.x == 0.0 {
                            areas[y] += (y as f32 - p0.y + 1.0).clamp(0.0, 1.0);
                        } else if p1.x == 0.0 {
                            areas[y] -= (y as f32 - p1.y + 1.0).clamp(0.0, 1.0);
                        }
                    }
                }
                let mut alphas = 0u32;
                for y in 0..4 {
                    let area = areas[y];
                    // nonzero winding number rule
                    let area_u8 = (area.abs().min(1.0) * 255.0).round() as u32;
                    alphas += area_u8 << (y * 8);
                }
                out.push(alphas);
            }
            if strip_start {
                let xy = (1 << 18) * prev_tile.y as u32 + 4 * prev_tile.x as u32 + x0;
                let strip = Strip {
                    path_id: tile.path_id,
                    xy,
                    col: cols,
                    winding: start_delta,
                };
                strips.push(strip);
            }
            cols += x1 - x0;
            fp = if same_strip { 1 } else { 0 };
            strip_start = !same_strip;
            seg_start = i;
            if !prev_tile.loc().same_row(&tile.loc()) {
                delta = 0;
            }
        }
        fp |= tile.footprint().0;
        prev_tile = tile;
    }
    (strips, out)
}

pub fn make_strips(lines: SoupBowl) -> (Vec<Strip>, Vec<u32>, Vec<PremulRgba8>) {
    let mut tiles = make_tiles(&lines.lines);
    tiles.sort_by(Tile::cmp);
    // This particular choice of sentinel tiles generates a sentinel strip.
    tiles.push(Tile {
        path_id: !1,
        x: 0x3fff,
        y: 0x3fff,
        p0: 0,
        p1: 0,
    });
    tiles.push(Tile {
        path_id: !0,
        x: 0x3fff,
        y: 0x3fff,
        p0: 0,
        p1: 0,
    });
    // for tile in &tiles {
    //     println!("{tile:?}");
    // }
    let (strips, alpha) = render_strips(&tiles);
    (strips, alpha, lines.colors)
}
