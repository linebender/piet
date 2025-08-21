// Copyright 2021 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! This example implements a generative algorithm for making pictures like
//! Piet Mondrian's squares.
// TODO: Remove all the wasm32 cfg guards once this compiles with piet-web

use piet::{Color, RenderContext, kurbo::Rect};
use piet_common::Device;
use piet_common::kurbo::{Point, Size};
use rand::{prelude::*, random};
use rand_distr::Normal;

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;
/// For now, assume pixel density (dots per inch)
const DPI: f64 = 96.;

/// Feature "png" needed for save_to_file() and it's disabled by default for optional dependencies
/// cargo run --example mondrian --features png
fn main() {
    let mut device = Device::new().unwrap();
    let mut bitmap = device.bitmap_target(WIDTH, HEIGHT, 1.0).unwrap();
    let mut rc = bitmap.render_context();
    Mondrian {
        split_count: ((WIDTH + HEIGHT) as f64 * 0.5 * 0.02) as usize,
        border: 0.5,
        min_gap: 0.3,
        skip_prob: 0.6,
        color_proportion: 0.2,
        stroke_width: 0.1,
        colors: vec![
            Color::BLACK,
            Color::rgb8(19, 86, 162),
            Color::rgb8(247, 216, 66),
            Color::rgb8(212, 9, 32),
        ],
        white: Color::rgb8(242, 245, 241),
    }
    .generate(Size::new(WIDTH as f64, HEIGHT as f64), &mut rc);

    rc.finish().unwrap();
    std::mem::drop(rc);

    bitmap
        .save_to_file("temp-image.png")
        .expect("file save error");
}

/// Generate a Piet Mondrian-style picture.
///
/// Obviously we cannot recreate the genius of an artist with a simple random
/// generation, but the following makes something plausible (heavily influenced by
/// https://generativeartistry.com/tutorials/piet-mondrian/):
///
///  1. Start with a single rectangle covering the whole picture.
///  2. Choose a number `n`. Choose `n` coordinates in the rectangle.
///  3. For each coordinate `p`: split any rectangles who are intersected by the
///     vertical or horizontal lines that cross `p`. Skip if this would create a
///     very thin rectangle, or randomly some of the time.
///  4. Color in a proportion (e.g. 1/6th) of the rectangles with the chosen colors.
///  5. Done :)
struct Mondrian {
    /// controls the number of splits of the canvas
    split_count: usize,
    /// border in inches
    border: f64,
    /// controls how close lines can be together, in inches
    min_gap: f64,
    /// controls how often a rect divide is skipped
    skip_prob: f64,
    /// controls what proportion of the rects will have color
    color_proportion: f64,
    /// the width of the lines between rectangles, in inches
    stroke_width: f64,
    /// the colors to use for coloring in squares
    colors: Vec<Color>,
    /// What to use for white (the majority of rects)
    white: Color,
}

impl Mondrian {
    fn generate(&self, size: Size, ctx: &mut impl RenderContext) {
        // Start with single rect over whole picture
        let mut rects = vec![ColorRect {
            color: self.white,
            rect: Rect::new(0., 0., size.width, size.height).inset(-self.border * DPI),
        }];
        let mut rng = thread_rng();

        // Split the rectangle `split_count` times.
        for _ in 0..self.split_count {
            let coord = Point {
                x: rng.r#gen::<f64>() * size.width,
                y: rng.r#gen::<f64>() * size.height,
            };

            rects = rects
                .into_iter()
                .flat_map(|rect| rect.intersect(coord, self.min_gap * DPI, self.skip_prob))
                .collect();
        }

        // color in some of the rectangles
        let proportion_each_color = self.color_proportion / self.colors.len() as f64;

        // Shuffle the rectangles so we are choosing them at random to color in.
        rects.shuffle(&mut rng);

        let mut i = 0;
        // use exp(normal) dist to select number of rects to color. I tried using ceil(num_rects)
        // but this colored too many when there were few rectangles.
        let n = Normal::new(
            // This controls the average number of colored in squares
            (proportion_each_color * rects.len() as f64).ln(),
            // This controls how much we are allowed to deviate from the average
            0.2f64,
        )
        .unwrap();
        'a: for color in &self.colors {
            for _ in 0..(n.sample(&mut rng).exp().round() as usize) {
                rects[i].color = *color;
                i += 1;
                // bail rather than panic if we run out of rectangles
                if i >= rects.len() {
                    break 'a;
                }
            }
        }

        // 2 passes so fills don't cover strokes.
        for rect in &rects {
            ctx.fill(rect.rect, &rect.color);
        }
        for rect in rects {
            ctx.stroke(rect.rect, &Color::BLACK, self.stroke_width * DPI);
        }
    }
}

/// A `Rect` with color information.
struct ColorRect {
    color: Color,
    rect: Rect,
}

impl ColorRect {
    /// Split `self` vertically and horizontally as needed given a point representing a vertical and
    /// a horizontal line
    fn intersect(self, p: Point, min_gap: f64, skip_prob: f64) -> impl Iterator<Item = ColorRect> {
        self.intersect_x(p.x, min_gap, skip_prob)
            .flat_map(move |rect| rect.intersect_y(p.y, min_gap, skip_prob))
    }

    /// Split `self` vertically about `x`.
    fn intersect_x(self, x: f64, min_gap: f64, skip_prob: f64) -> impl Iterator<Item = ColorRect> {
        if self.rect.x0 + min_gap < x
            && x < self.rect.x1 - min_gap
            && random::<f64>() <= (1. - skip_prob)
        {
            Either::Left(IntoIterator::into_iter([
                ColorRect {
                    color: self.color,
                    rect: Rect {
                        x0: self.rect.x0,
                        y0: self.rect.y0,
                        x1: x,
                        y1: self.rect.y1,
                    },
                },
                ColorRect {
                    color: self.color,
                    rect: Rect {
                        x0: x,
                        y0: self.rect.y0,
                        x1: self.rect.x1,
                        y1: self.rect.y1,
                    },
                },
            ]))
        } else {
            Either::Right(IntoIterator::into_iter([self]))
        }
    }

    /// Split `self` horizontally about `y`.
    fn intersect_y(self, y: f64, min_gap: f64, skip_prob: f64) -> impl Iterator<Item = ColorRect> {
        if self.rect.y0 + min_gap < y
            && y < self.rect.y1 - min_gap
            && random::<f64>() <= (1. - skip_prob)
        {
            Either::Left(IntoIterator::into_iter([
                ColorRect {
                    color: self.color,
                    rect: Rect {
                        x0: self.rect.x0,
                        y0: self.rect.y0,
                        x1: self.rect.x1,
                        y1: y,
                    },
                },
                ColorRect {
                    color: self.color,
                    rect: Rect {
                        x0: self.rect.x0,
                        y0: y,
                        x1: self.rect.x1,
                        y1: self.rect.y1,
                    },
                },
            ]))
        } else {
            Either::Right(IntoIterator::into_iter([self]))
        }
    }
}

/// This just allows me to return an iterator that might be one
/// of 2 concrete types.
enum Either<T, U> {
    Left(T),
    Right(U),
}

impl<Item, T, U> Iterator for Either<T, U>
where
    T: Iterator<Item = Item>,
    U: Iterator<Item = Item>,
{
    type Item = Item;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Either::Left(t) => t.next(),
            Either::Right(u) => u.next(),
        }
    }
}
