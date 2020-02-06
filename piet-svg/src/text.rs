use piet::kurbo::Point;
use piet::{new_error, Error, ErrorKind, HitTestPoint, HitTestTextPosition};

type Result<T> = std::result::Result<T, Error>;

/// SVG text (unimplemented)
pub struct Text(());

impl Text {
    pub fn new() -> Self {
        Text(())
    }
}

impl piet::Text for Text {
    type Font = Font;
    type FontBuilder = FontBuilder;
    type TextLayout = TextLayout;
    type TextLayoutBuilder = TextLayoutBuilder;

    fn new_font_by_name(&mut self, _name: &str, _size: f64) -> FontBuilder {
        FontBuilder(())
    }

    fn new_text_layout(&mut self, _font: &Self::Font, _text: &str) -> TextLayoutBuilder {
        TextLayoutBuilder(())
    }
}

/// SVG font builder (unimplemented)
pub struct FontBuilder(());

impl piet::FontBuilder for FontBuilder {
    type Out = Font;

    fn build(self) -> Result<Font> {
        Err(new_error(ErrorKind::NotSupported))
    }
}

/// SVG font (unimplemented)
pub struct Font(());

impl piet::Font for Font {}

pub struct TextLayoutBuilder(());

impl piet::TextLayoutBuilder for TextLayoutBuilder {
    type Out = TextLayout;

    fn build(self) -> Result<TextLayout> {
        Err(new_error(ErrorKind::NotSupported))
    }
}

/// SVG text layout (unimplemented)
pub struct TextLayout(());

impl piet::TextLayout for TextLayout {
    fn width(&self) -> f64 {
        unimplemented!()
    }

    fn hit_test_point(&self, _point: Point) -> HitTestPoint {
        unimplemented!()
    }

    fn hit_test_text_position(&self, _text_position: usize) -> Option<HitTestTextPosition> {
        unimplemented!()
    }
}
