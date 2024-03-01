// Copyright 2020 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Using custom font families.

use crate::kurbo::{Size, Vec2};
use crate::{
    Color, Error, FontFamily, FontStyle, FontWeight, RenderContext, Text, TextLayout,
    TextLayoutBuilder,
};

pub const SIZE: Size = Size::new(240., 280.);

static TEXT: &str = r#"Philosophers often behave like little children who scribble some marks on a piece of paper at random and then ask the grown-up "What's that?" — It happened like this: the grown-up had drawn pictures for the child several times and said "this is a man," "this is a house," etc. And then the child makes some marks too and asks: what's this then?"#;

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(None, Color::WHITE);
    let text = rc.text();
    let _ = text.load_font(include_bytes!(
        "../../snapshots/resources/Anaheim-Regular.ttf"
    )); // SEEING AN ERROR? run `git submodule update --init`
    let font = text
        .load_font(include_bytes!("../../snapshots/resources/Anaheim-Bold.ttf"))
        .unwrap_or(FontFamily::SYSTEM_UI);

    let layout = text
        .new_text_layout(TEXT)
        .max_width(200.0)
        .font(font, 12.0)
        // this should resolve to a mono font; ensure font is found on windows
        .range_attribute(..30, FontFamily::MONOSPACE)
        // weight doesn't exist; this should resolve to bold
        .range_attribute(100..150, FontWeight::EXTRA_BLACK)
        .range_attribute(150..250, FontWeight::BOLD)
        // italic does not exist. should be synthetic italic?
        .range_attribute(200..300, FontStyle::Italic)
        // weight does not exist; should resolve to regular
        .range_attribute(250..320, FontWeight::EXTRA_LIGHT)
        .build()?;

    let y_pos = ((SIZE.height - layout.size().height) / 2.0).max(0.0);
    let text_pos = Vec2::new(16.0, y_pos);
    rc.draw_text(&layout, text_pos.to_point());

    Ok(())
}
