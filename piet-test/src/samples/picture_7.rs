//! Text layouts

use piet::kurbo::{Rect, Size};
use piet::{Color, Error, RenderContext, Text, TextAlignment, TextAttribute, TextLayoutBuilder};

pub const SIZE: Size = Size::new(800., 800.);
static SAMPLE_EN: &str = r#"This essay is an effort to build an ironic political myth faithful to feminism, socialism, and materialism. Perhaps more faithful as blasphemy is faithful, than as reverent worship and identification. Blasphemy has always seemed to require taking things very seriously. I know no better stance to adopt from within the secular-religious, evangelical traditions of United States politics, including the politics of socialist-feminism."#;

static SAMPLE_AR: &str = r#"لكن لا بد أن أوضح لك أن كل هذه الأفكار المغلوطة حول استنكار  النشوة وتمجيد الألم نشأت بالفعل، وسأعرض لك التفاصيل لتكتشف حقيقة وأساس تلك السعادة البشرية، فلا أحد يرفض أو يكره أو يتجنب الشعور بالسعادة، ولكن بفضل هؤلاء الأشخاص الذين لا يدركون بأن السعادة لا بد أن نستشعرها بصورة أكثر عقلانية ومنطقية فيعرضهم هذا لمواجهة الظروف الأليمة، وأكرر بأنه لا يوجد من يرغب في الحب ونيل المنال ويتلذذ بالآلام، الألم هو الألم ولكن نتيجة لظروف ما قد تكمن السعاده فيما نتحمله من كد وأسي."#;

const LIGHT_GREY: Color = Color::grey8(0xF0);

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(None, Color::WHITE);
    let text = rc.text();

    let en_leading = text
        .new_text_layout(SAMPLE_EN)
        .alignment(TextAlignment::Start)
        .max_width(100.0)
        .default_attribute(TextAttribute::FontSize(8.0))
        .build()?;

    let en_trailing = text
        .new_text_layout(SAMPLE_EN)
        .alignment(TextAlignment::End)
        .max_width(100.0)
        .default_attribute(TextAttribute::FontSize(8.0))
        .build()?;

    let en_center = text
        .new_text_layout(SAMPLE_EN)
        .alignment(TextAlignment::Center)
        .max_width(100.0)
        .default_attribute(TextAttribute::FontSize(8.0))
        .build()?;

    let en_justify = text
        .new_text_layout(SAMPLE_EN)
        .alignment(TextAlignment::Justified)
        .max_width(100.0)
        .default_attribute(TextAttribute::FontSize(8.0))
        .build()?;

    let ar_leading = text
        .new_text_layout(SAMPLE_AR)
        .alignment(TextAlignment::Start)
        .max_width(100.0)
        .default_attribute(TextAttribute::FontSize(8.0))
        .build()?;

    let ar_trailing = text
        .new_text_layout(SAMPLE_AR)
        .alignment(TextAlignment::End)
        .max_width(100.0)
        .default_attribute(TextAttribute::FontSize(8.0))
        .build()?;

    let ar_center = text
        .new_text_layout(SAMPLE_AR)
        .alignment(TextAlignment::Center)
        .max_width(100.0)
        .default_attribute(TextAttribute::FontSize(8.0))
        .build()?;

    let ar_justify = text
        .new_text_layout(SAMPLE_AR)
        .alignment(TextAlignment::Justified)
        .max_width(100.0)
        .default_attribute(TextAttribute::FontSize(8.0))
        .build()?;

    for pt in &[(0f64, 0f64), (200.0, 0.), (100., 200.), (300., 200.)] {
        let rect = Rect::from_origin_size(*pt, (100., 200.));
        rc.fill(rect, &LIGHT_GREY);
    }

    rc.draw_text(&en_leading, (0., 0.));
    rc.draw_text(&en_trailing, (100., 0.));
    rc.draw_text(&en_center, (200., 0.));
    rc.draw_text(&en_justify, (300., 0.));

    rc.draw_text(&ar_leading, (0., 200.));
    rc.draw_text(&ar_trailing, (100., 200.));
    rc.draw_text(&ar_center, (200., 200.));
    rc.draw_text(&ar_justify, (300., 200.));

    Ok(())
}
