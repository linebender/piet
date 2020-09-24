//! Styled text

use crate::kurbo::Size;
use crate::{
    Color, Error, FontFamily, FontStyle, FontWeight, RenderContext, Text, TextAlignment,
    TextAttribute, TextLayoutBuilder,
};

pub const SIZE: Size = Size::new(400., 800.);

static SAMPLE_EN: &str = r#"This essay is an effort to build an ironic political myth faithful to feminism, socialism, and materialism. Perhaps more faithful as blasphemy is faithful, than as reverent worship and identification. Blasphemy has always seemed to require taking things very seriously. I know no better stance to adopt from within the secular-religious, evangelical traditions of United States politics, including the politics of socialist-feminism."#;

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(Color::WHITE);
    let text = rc.text();

    let en_leading = text
        .new_text_layout(SAMPLE_EN)
        .max_width(200.0)
        .default_attribute(FontFamily::SYSTEM_UI)
        .alignment(TextAlignment::Start)
        .range_attribute(10..80, TextAttribute::FontSize(8.0))
        .range_attribute(20..120, FontFamily::SERIF)
        .range_attribute(40..60, FontWeight::BOLD)
        .range_attribute(60..140, FontWeight::THIN)
        .range_attribute(90..300, FontFamily::MONOSPACE)
        .range_attribute(120..150, TextAttribute::TextColor(Color::rgb(0.6, 0., 0.)))
        .range_attribute(160..190, TextAttribute::TextColor(Color::rgb(0., 0.6, 0.)))
        .range_attribute(200..240, TextAttribute::TextColor(Color::rgb(0., 0., 0.6)))
        .range_attribute(200.., FontWeight::EXTRA_BLACK)
        .range_attribute(220.., TextAttribute::FontSize(18.0))
        .range_attribute(240.., FontStyle::Italic)
        .range_attribute(280.., TextAttribute::Underline(true))
        .range_attribute(320.., TextAttribute::Strikethrough(true))
        .build()?;

    rc.draw_text(&en_leading, (0., 0.));

    Ok(())
}
