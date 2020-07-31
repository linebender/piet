//! Styled text

use crate::kurbo::Size;
use crate::{
    Color, Error, FontBuilder, FontWeight, RenderContext, Text, TextAlignment, TextAttribute,
    TextLayoutBuilder,
};

pub const SIZE: Size = Size::new(400., 800.);

static SAMPLE_EN: &str = r#"This essay is an effort to build an ironic political myth faithful to feminism, socialism, and materialism. Perhaps more faithful as blasphemy is faithful, than as reverent worship and identification. Blasphemy has always seemed to require taking things very seriously. I know no better stance to adopt from within the secular-religious, evangelical traditions of United States politics, including the politics of socialist-feminism."#;

const SERIF: &str = "Times New Roman";
#[cfg(target_os = "windows")]
const MONO: &str = "Courier New";
#[cfg(not(target_os = "windows"))]
const MONO: &str = "Courier";

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(Color::WHITE);
    let text = rc.text();
    let font = text.system_font(12.0);
    let serif = text.new_font_by_name(SERIF, 20.0).build().unwrap();
    let mono = text.new_font_by_name(MONO, 12.0).build().unwrap();

    let en_leading = text
        .new_text_layout(SAMPLE_EN)
        .max_width(200.0)
        .default_attribute(font)
        .alignment(TextAlignment::Start)
        .range_attribute(10..80, TextAttribute::Size(8.0))
        .range_attribute(20..120, serif)
        .range_attribute(40..60, FontWeight::BOLD)
        .range_attribute(60..140, FontWeight::THIN)
        .range_attribute(90..300, mono)
        .range_attribute(
            120..150,
            TextAttribute::ForegroundColor(Color::rgb(0.6, 0., 0.)),
        )
        .range_attribute(
            160..190,
            TextAttribute::ForegroundColor(Color::rgb(0., 0.6, 0.)),
        )
        .range_attribute(
            200..240,
            TextAttribute::ForegroundColor(Color::rgb(0., 0., 0.6)),
        )
        .range_attribute(200.., FontWeight::EXTRA_BLACK)
        .range_attribute(220.., TextAttribute::Size(18.0))
        .range_attribute(240.., TextAttribute::Italic(true))
        .range_attribute(280.., TextAttribute::Underline(true))
        .build()?;

    rc.draw_text(&en_leading, (0., 0.));

    Ok(())
}
