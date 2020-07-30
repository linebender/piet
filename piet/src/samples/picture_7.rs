//! Text layouts

use crate::kurbo::Size;
use crate::{Color, Error, RenderContext, Text, TextAlignment, TextAttribute, TextLayoutBuilder};

pub const SIZE: Size = Size::new(800., 800.);
static SAMPLE_EN: &str = r#"
This essay is an effort to build an ironic political myth faithful to feminism, socialism, and materialism. Perhaps more faithful as blasphemy is faithful, than as reverent worship and identification. Blasphemy has always seemed to require taking things very seriously. I know no better stance to adopt from within the secular-religious, evangelical traditions of United States politics, including the politics of socialist-feminism."#;

static SAMPLE_AR: &str = r#"لكن لا بد أن أوضح لك أن كل هذه الأفكار المغلوطة حول استنكار  النشوة وتمجيد الألم نشأت بالفعل، وسأعرض لك التفاصيل لتكتشف حقيقة وأساس تلك السعادة البشرية، فلا أحد يرفض أو يكره أو يتجنب الشعور بالسعادة، ولكن بفضل هؤلاء الأشخاص الذين لا يدركون بأن السعادة لا بد أن نستشعرها بصورة أكثر عقلانية ومنطقية فيعرضهم هذا لمواجهة الظروف الأليمة، وأكرر بأنه لا يوجد من يرغب في الحب ونيل المنال ويتلذذ بالآلام، الألم هو الألم ولكن نتيجة لظروف ما قد تكمن السعاده فيما نتحمله من كد وأسي.
"#;

pub fn draw<R: RenderContext>(rc: &mut R) -> Result<(), Error> {
    rc.clear(Color::WHITE);
    let text = rc.text();
    let font = text.system_font(8.0);

    let en_leading = text
        .new_text_layout(&font, SAMPLE_EN, 100.0)
        .alignment(TextAlignment::Start)
        .add_attribute(.., TextAttribute::Size(8.0))
        .build()?;

    let en_trailing = text
        .new_text_layout(&font, SAMPLE_EN, 100.0)
        .alignment(TextAlignment::End)
        .build()?;

    let en_center = text
        .new_text_layout(&font, SAMPLE_EN, 100.0)
        .alignment(TextAlignment::Center)
        .build()?;

    let en_justify = text
        .new_text_layout(&font, SAMPLE_EN, 100.0)
        .alignment(TextAlignment::Justified)
        .build()?;

    let ar_leading = text
        .new_text_layout(&font, SAMPLE_AR, 100.0)
        .alignment(TextAlignment::Start)
        .build()?;

    let ar_trailing = text
        .new_text_layout(&font, SAMPLE_AR, 100.0)
        .alignment(TextAlignment::End)
        .build()?;

    let ar_center = text
        .new_text_layout(&font, SAMPLE_AR, 100.0)
        .alignment(TextAlignment::Center)
        .build()?;

    let ar_justify = text
        .new_text_layout(&font, SAMPLE_AR, 100.0)
        .alignment(TextAlignment::Justified)
        .build()?;

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
