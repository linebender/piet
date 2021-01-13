//! Text functionality for Piet cairo backend

mod grapheme;
mod lines;

use std::convert::TryInto;
use std::fmt;
use std::ops::{Range, RangeBounds};
use std::rc::Rc;

use cairo::{FontFace, FontOptions, FontSlant, FontWeight, Matrix, ScaledFont};

use pango::{AttrList, FontMapExt};
use pangocairo::FontMap;

use piet::kurbo::{Point, Rect, Size};
use piet::{
    util, Error, FontFamily, FontStyle, HitTestPoint, HitTestPosition, LineMetric, Text,
    TextAttribute, TextLayout, TextLayoutBuilder, TextStorage,
};

use unicode_segmentation::UnicodeSegmentation;

use self::grapheme::{get_grapheme_boundaries, point_x_in_grapheme};

type PangoLayout = pango::Layout;
type PangoContext = pango::Context;
type PangoAttribute = pango::Attribute;

/// Right now, we don't need any state, as the "toy text API" treats the
/// access to system font information as a global. This will change.
// we use a phantom lifetime here to match the API of the d2d backend,
// and the likely API of something with access to system font information.
#[derive(Clone)]
pub struct CairoText {
    pango_context: PangoContext,
}

#[derive(Clone)]
struct CairoFont {
    family: FontFamily,
}

#[derive(Clone)]
pub struct CairoTextLayout {
    size: Size,
    trailing_ws_width: f64,
    pub(crate) font: ScaledFont,
    pub(crate) text: Rc<dyn TextStorage>,

    // currently calculated on build
    pub(crate) line_metrics: Vec<LineMetric>,

    pub(crate) pango_layout: PangoLayout,
}

pub struct CairoTextLayoutBuilder {
    text: Rc<dyn TextStorage>,
    defaults: util::LayoutDefaults,
    attributes: Vec<AttributeWithRange>,
    last_range_start_pos: usize,
    width_constraint: f64,
    pango_layout: PangoLayout,
}

pub struct AttributeWithRange {
    range: Range<usize>,
    attribute: TextAttribute,
}

impl CairoText {
    /// Create a new factory that satisfies the piet `Text` trait.
    ///
    /// No state is needed for now because the current implementation is just
    /// toy text, but that will change when proper text is implemented.
    #[allow(clippy::new_without_default)]
    pub fn new() -> CairoText {
        let fontmap = FontMap::get_default().unwrap();
        CairoText {
            pango_context: fontmap.create_context().unwrap(),
        }
    }
}

impl Text for CairoText {
    type TextLayout = CairoTextLayout;
    type TextLayoutBuilder = CairoTextLayoutBuilder;

    fn font_family(&mut self, family_name: &str) -> Option<FontFamily> {
        Some(FontFamily::new_unchecked(family_name))
    }

    fn load_font(&mut self, _data: &[u8]) -> Result<FontFamily, Error> {
        Err(Error::NotSupported)
    }

    fn new_text_layout(&mut self, text: impl TextStorage) -> Self::TextLayoutBuilder {
        let pango_layout = PangoLayout::new(&self.pango_context);
        pango_layout.set_text(text.as_str());

        CairoTextLayoutBuilder {
            text: Rc::new(text),
            defaults: util::LayoutDefaults::default(),
            attributes: Vec::new(),
            last_range_start_pos: 0,
            width_constraint: f64::INFINITY,
            pango_layout,
        }
    }
}

impl fmt::Debug for CairoText {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CairoText").finish()
    }
}

impl CairoFont {
    pub(crate) fn new(family: FontFamily) -> Self {
        CairoFont { family }
    }

    #[cfg(test)]
    pub(crate) fn resolve_simple(&self, size: f64) -> ScaledFont {
        self.resolve(size, FontSlant::Normal, FontWeight::Normal)
    }

    /// Create a ScaledFont for this family.
    pub(crate) fn resolve(&self, size: f64, slant: FontSlant, weight: FontWeight) -> ScaledFont {
        let font_face = FontFace::toy_create(self.family.name(), slant, weight);
        let font_matrix = scale_matrix(size);
        let ctm = scale_matrix(1.0);
        let options = FontOptions::default();
        ScaledFont::new(&font_face, &font_matrix, &ctm, &options)
    }
}

impl fmt::Debug for CairoFont {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CairoFont").finish()
    }
}

impl TextLayoutBuilder for CairoTextLayoutBuilder {
    type Out = CairoTextLayout;

    fn max_width(mut self, width: f64) -> Self {
        self.width_constraint = width;
        self
    }

    fn alignment(self, _alignment: piet::TextAlignment) -> Self {
        // TextAlignment is not supported by cairo toy text.
        self
    }

    fn default_attribute(mut self, attribute: impl Into<TextAttribute>) -> Self {
        self.defaults.set(attribute);
        self
    }

    fn range_attribute(
        mut self,
        range: impl RangeBounds<usize>,
        attribute: impl Into<TextAttribute>,
    ) -> Self {
        let range = util::resolve_range(range, self.text.len());
        let attribute = attribute.into();

        debug_assert!(
            range.start >= self.last_range_start_pos,
            "attributes must be added in non-decreasing start order"
        );
        self.last_range_start_pos = range.start;

        self.attributes
            .push(AttributeWithRange { range, attribute });

        self
    }

    fn build(self) -> Result<Self::Out, Error> {
        // set our default font
        let font = CairoFont::new(self.defaults.font.clone());
        let size = self.defaults.font_size;
        let weight = if self.defaults.weight.to_raw() <= piet::FontWeight::MEDIUM.to_raw() {
            FontWeight::Normal
        } else {
            FontWeight::Bold
        };
        let slant = match self.defaults.style {
            FontStyle::Italic => FontSlant::Italic,
            FontStyle::Regular => FontSlant::Normal,
        };

        let scaled_font = font.resolve(size, slant, weight);

        let pango_attributes = AttrList::new();

        pango_attributes.insert({
            let (r, g, b, _) = self.defaults.fg_color.as_rgba();
            PangoAttribute::new_foreground(
                (r * 65535.) as u16,
                (g * 65535.) as u16,
                (b * 65535.) as u16,
            )
            .unwrap()
        });

        pango_attributes.insert({
            let font = self.defaults.font;
            PangoAttribute::new_family(font.name()).unwrap()
        });

        for attribute in self.attributes {
            match &attribute.attribute {
                TextAttribute::TextColor(text_color) => {
                    let (r, g, b, _) = text_color.as_rgba();
                    let mut pango_attribute = PangoAttribute::new_foreground(
                        (r * 65535.) as u16,
                        (g * 65535.) as u16,
                        (b * 65535.) as u16,
                    )
                    .unwrap();
                    pango_attribute.set_start_index(attribute.range.start.try_into().unwrap());
                    pango_attribute.set_end_index(attribute.range.end.try_into().unwrap());
                    pango_attributes.insert(pango_attribute);
                }

                TextAttribute::FontSize(_) => {}

                _ => {}
            }
        }

        self.pango_layout.set_attributes(Some(&pango_attributes));

        //NOTE: We give Pango a width of -1 for no wrapping
        self.pango_layout.set_wrap(pango::WrapMode::Word);
        self.pango_layout.set_ellipsize(pango::EllipsizeMode::None);

        // invalid until update_width() is called
        let mut layout = CairoTextLayout {
            font: scaled_font,
            size: Size::ZERO,
            trailing_ws_width: 0.0,
            line_metrics: Vec::new(),
            text: self.text,
            pango_layout: self.pango_layout,
        };

        layout.update_width(self.width_constraint);
        Ok(layout)
    }
}

impl fmt::Debug for CairoTextLayoutBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CairoTextLayoutBuilder").finish()
    }
}

impl TextLayout for CairoTextLayout {
    fn size(&self) -> Size {
        self.size
    }

    fn trailing_whitespace_width(&self) -> f64 {
        self.trailing_ws_width
    }

    fn image_bounds(&self) -> Rect {
        self.size.to_rect()
    }

    fn text(&self) -> &str {
        &self.text
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        self.line_metrics
            .get(line_number)
            .map(|lm| &self.text[lm.range()])
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        self.line_metrics.get(line_number).cloned()
    }

    fn line_count(&self) -> usize {
        self.line_metrics.len()
    }

    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        // internal logic is using grapheme clusters, but return the text position associated
        // with the border of the grapheme cluster.

        // null case
        if self.text.is_empty() {
            return HitTestPoint::default();
        }

        let height = self
            .line_metrics
            .last()
            .map(|lm| lm.y_offset + lm.height)
            .unwrap_or(0.0);

        // determine whether this click is within the y bounds of the layout,
        // and what line it coorresponds to. (For points above and below the layout,
        // we hittest the first and last lines respectively.)
        let (y_inside, lm) = if point.y < 0. {
            (false, self.line_metrics.first().unwrap())
        } else if point.y >= height {
            (false, self.line_metrics.last().unwrap())
        } else {
            let line = self
                .line_metrics
                .iter()
                .find(|l| point.y >= l.y_offset && point.y < l.y_offset + l.height)
                .unwrap();
            (true, line)
        };

        // Trailing whitespace is remove for the line
        let line = &self.text[lm.range()];

        let mut htp = hit_test_line_point(&self.font, line, point);
        htp.idx += lm.start_offset;
        if htp.idx == lm.end_offset {
            htp.idx -= util::trailing_nlf(line).unwrap_or(0);
        }
        htp.is_inside &= y_inside;
        htp
    }

    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition {
        let idx = idx.min(self.text.len());
        assert!(self.text.is_char_boundary(idx));

        if idx == 0 && self.text.is_empty() {
            return HitTestPosition::new(Point::new(0., self.font.extents().ascent), 0);
        }

        // first need to find line it's on, and get line start offset
        let line_num = util::line_number_for_position(&self.line_metrics, idx);
        let lm = self.line_metrics.get(line_num).cloned().unwrap();

        let y_pos = lm.y_offset + lm.baseline;

        // Then for the line, do text position
        // Trailing whitespace is removed for the line
        let line = &self.text[lm.range()];
        let line_position = idx - lm.start_offset;

        let x_pos = hit_test_line_position(&self.font, line, line_position);
        HitTestPosition::new(Point::new(x_pos, y_pos), line_num)
    }
}

impl CairoTextLayout {
    fn update_width(&mut self, new_width: impl Into<Option<f64>>) {
        let new_width = if let Some(new_width) = new_width.into() {
            let pango_width = new_width * pango::SCALE as f64;
            self.pango_layout.set_width(pango_width as i32);
            new_width
        } else {
            //NOTE: -1 is the default value, however `update_width` *could*
            //be called any number of times with different values so we need
            //to make sure to reset back to default whenever we get no width
            self.pango_layout.set_width(-1);
            std::f64::INFINITY
        };

        self.line_metrics = lines::calculate_line_metrics(&self.text, &self.font, new_width);
        if self.text.is_empty() {
            self.line_metrics.push(LineMetric {
                baseline: self.font.extents().ascent,
                height: self.font.extents().height,
                ..Default::default()
            })
        } else if util::trailing_nlf(&self.text).is_some() {
            let newline_eof = self
                .line_metrics
                .last()
                .map(|lm| LineMetric {
                    start_offset: self.text.len(),
                    end_offset: self.text.len(),
                    height: lm.height,
                    baseline: lm.baseline,
                    y_offset: lm.y_offset + lm.height,
                    trailing_whitespace: 0,
                })
                .unwrap();
            self.line_metrics.push(newline_eof);
        }

        let (width, ws_width) = self
            .line_metrics
            .iter()
            .map(|lm| {
                let full_width = self.font.text_extents(&self.text[lm.range()]).x_advance;
                let non_ws_width = if lm.trailing_whitespace > 0 {
                    let non_ws_range = lm.start_offset..lm.end_offset - lm.trailing_whitespace;
                    self.font.text_extents(&self.text[non_ws_range]).x_advance
                } else {
                    full_width
                };
                (non_ws_width, full_width)
            })
            .fold((0.0, 0.0), |a: (f64, f64), b| (a.0.max(b.0), a.1.max(b.1)));

        let height = self
            .line_metrics
            .last()
            .map(|l| l.y_offset + l.height)
            .unwrap_or_else(|| self.font.extents().height);
        self.size = Size::new(width, height);
        self.trailing_ws_width = ws_width;
    }
}

impl fmt::Debug for CairoTextLayout {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CairoTextLayout").finish()
    }
}

// NOTE this is the same as the old, non-line-aware version of hit_test_point
// Future: instead of passing Font, should there be some other line-level text layout?
fn hit_test_line_point(font: &ScaledFont, text: &str, point: Point) -> HitTestPoint {
    // null case
    if text.is_empty() {
        return HitTestPoint::default();
    }

    // get bounds
    // TODO handle if string is not null yet count is 0?
    let end = UnicodeSegmentation::graphemes(text, true).count() - 1;
    let end_bounds = match get_grapheme_boundaries(font, text, end) {
        Some(bounds) => bounds,
        None => return HitTestPoint::default(),
    };

    let start = 0;
    let start_bounds = match get_grapheme_boundaries(font, text, start) {
        Some(bounds) => bounds,
        None => return HitTestPoint::default(),
    };

    // first test beyond ends
    if point.x > end_bounds.trailing {
        return HitTestPoint::new(text.len(), false);
    }
    if point.x <= start_bounds.leading {
        return HitTestPoint::default();
    }

    // then test the beginning and end (common cases)
    if let Some(hit) = point_x_in_grapheme(point.x, &start_bounds) {
        return hit;
    }
    if let Some(hit) = point_x_in_grapheme(point.x, &end_bounds) {
        return hit;
    }

    // Now that we know it's not beginning or end, begin binary search.
    // Iterative style
    let mut left = start;
    let mut right = end;
    loop {
        // pick halfway point
        let middle = left + ((right - left) / 2);

        let grapheme_bounds = match get_grapheme_boundaries(font, text, middle) {
            Some(bounds) => bounds,
            None => return HitTestPoint::default(),
        };

        if let Some(hit) = point_x_in_grapheme(point.x, &grapheme_bounds) {
            return hit;
        }

        // since it's not a hit, check if closer to start or finish
        // and move the appropriate search boundary
        if point.x < grapheme_bounds.leading {
            right = middle;
        } else if point.x > grapheme_bounds.trailing {
            left = middle + 1;
        } else {
            unreachable!("hit_test_point conditional is exhaustive");
        }
    }
}

// NOTE this is the same as the old, non-line-aware version of hit_test_text_position.
// Future: instead of passing Font, should there be some other line-level text layout?
fn hit_test_line_position(font: &ScaledFont, text: &str, text_position: usize) -> f64 {
    // Using substrings with unicode grapheme awareness

    let text_len = text.len();

    if text_position == 0 {
        return 0.0;
    }

    if text_position as usize >= text_len {
        return font.text_extents(&text).x_advance;
    }

    // Already checked that text_position > 0 and text_position < count.
    // If text position is not at a grapheme boundary, use the text position of current
    // grapheme cluster. But return the original text position
    // Use the indices (byte offset, which for our purposes = utf8 code units).
    let grapheme_indices = UnicodeSegmentation::grapheme_indices(text, true)
        .take_while(|(byte_idx, _s)| text_position >= *byte_idx);

    grapheme_indices
        .last()
        .map(|(idx, _)| font.text_extents(&text[..idx]).x_advance)
        .unwrap_or_else(|| font.text_extents(&text).x_advance)
}

fn scale_matrix(scale: f64) -> Matrix {
    Matrix {
        xx: scale,
        yx: 0.0,
        xy: 0.0,
        yy: scale,
        x0: 0.0,
        y0: 0.0,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use piet::TextLayout;

    macro_rules! assert_close {
        ($val:expr, $target:expr, $tolerance:expr) => {{
            let min = $target - $tolerance;
            let max = $target + $tolerance;
            if $val < min || $val > max {
                panic!(
                    "value {} outside target {} with tolerance {}",
                    $val, $target, $tolerance
                );
            }
        }};

        ($val:expr, $target:expr, $tolerance:expr,) => {{
            assert_close!($val, $target, $tolerance)
        }};
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn hit_test_empty_string() {
        let layout = CairoText::new().new_text_layout("").build().unwrap();
        let pt = layout.hit_test_point(Point::new(0.0, 0.0));
        assert_eq!(pt.idx, 0);
        let pos = layout.hit_test_text_position(0);
        assert_eq!(pos.point.x, 0.0);
        assert_close!(pos.point.y, 10.0, 3.0);
        let line = layout.line_metric(0).unwrap();
        assert_close!(line.height, 12.0, 3.0);
    }

    #[test]
    fn test_hit_test_text_position_basic() {
        let mut text_layout = CairoText::new();

        let input = "piet text!";

        let layout = text_layout.new_text_layout(&input[0..4]).build().unwrap();
        let piet_width = layout.size().width;

        let layout = text_layout.new_text_layout(&input[0..3]).build().unwrap();
        let pie_width = layout.size().width;

        let layout = text_layout.new_text_layout(&input[0..2]).build().unwrap();
        let pi_width = layout.size().width;

        let layout = text_layout.new_text_layout(&input[0..1]).build().unwrap();
        let p_width = layout.size().width;

        let layout = text_layout.new_text_layout("").build().unwrap();
        let null_width = layout.size().width;

        let full_layout = text_layout.new_text_layout(input).build().unwrap();
        let full_width = full_layout.size().width;

        assert_close!(
            full_layout.hit_test_text_position(4).point.x,
            piet_width,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(3).point.x,
            pie_width,
            3.0,
        );
        assert_close!(full_layout.hit_test_text_position(2).point.x, pi_width, 3.0,);
        assert_close!(full_layout.hit_test_text_position(1).point.x, p_width, 3.0,);
        assert_close!(
            full_layout.hit_test_text_position(0).point.x,
            null_width,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(10).point.x,
            full_width,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(11).point.x,
            full_width,
            3.0,
        );
    }

    #[test]
    fn test_hit_test_text_position_complex_0() {
        let input = "é";
        assert_eq!(input.len(), 2);

        let mut text_layout = CairoText::new();
        let layout = text_layout.new_text_layout(input).build().unwrap();

        assert_close!(layout.hit_test_text_position(0).point.x, 0.0, 3.0);
        assert_close!(
            layout.hit_test_text_position(2).point.x,
            layout.size().width,
            3.0,
        );

        // unicode segmentation is wrong on this one for now.
        //let input = "🤦\u{1f3fc}\u{200d}\u{2642}\u{fe0f}";

        //let mut text_layout = D2DText::new();
        //let font = text_layout.new_font_by_name("sans-serif", 12.0).build().unwrap();
        //let layout = text_layout.new_text_layout(&font, input, std::f64::INFINITY).build().unwrap();

        //assert_eq!(input.graphemes(true).count(), 1);
        //assert_eq!(layout.hit_test_text_position(0, true).map(|p| p.point_x as f64), Some(layout.size().width));
        //assert_eq!(input.len(), 17);

        let input = "\u{0023}\u{FE0F}\u{20E3}"; // #️⃣
        assert_eq!(input.len(), 7);
        assert_eq!(input.chars().count(), 3);

        let mut text_layout = CairoText::new();
        let layout = text_layout.new_text_layout(input).build().unwrap();

        assert_close!(layout.hit_test_text_position(0).point.x, 0.0, 3.0);
        assert_close!(
            layout.hit_test_text_position(7).point.x,
            layout.size().width,
            3.0,
        );
    }

    #[test]
    fn test_hit_test_text_position_complex_1() {
        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇
        assert_eq!(input.len(), 14);

        let mut text_layout = CairoText::new();
        let layout = text_layout.new_text_layout(input).build().unwrap();

        let test_layout_0 = text_layout.new_text_layout(&input[0..2]).build().unwrap();
        let test_layout_1 = text_layout.new_text_layout(&input[0..9]).build().unwrap();
        let test_layout_2 = text_layout.new_text_layout(&input[0..10]).build().unwrap();

        // Note: text position is in terms of utf8 code units
        assert_close!(layout.hit_test_text_position(0).point.x, 0.0, 3.0);
        assert_close!(
            layout.hit_test_text_position(2).point.x,
            test_layout_0.size().width,
            3.0,
        );
        assert_close!(
            layout.hit_test_text_position(9).point.x,
            test_layout_1.size().width,
            3.0,
        );
        assert_close!(
            layout.hit_test_text_position(10).point.x,
            test_layout_2.size().width,
            3.0,
        );
        assert_close!(
            layout.hit_test_text_position(14).point.x,
            layout.size().width,
            3.0,
        );

        // Code point boundaries, but not grapheme boundaries.
        // Width should stay at the current grapheme boundary.
        assert_close!(
            layout.hit_test_text_position(3).point.x,
            test_layout_0.size().width,
            3.0,
        );
        assert_close!(
            layout.hit_test_text_position(6).point.x,
            test_layout_0.size().width,
            3.0,
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_hit_test_point_basic_0() {
        let mut text_layout = CairoText::new();

        let layout = text_layout.new_text_layout("piet text!").build().unwrap();
        println!("text pos 4: {:?}", layout.hit_test_text_position(4)); // 23.0
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 27.0

        // test hit test point
        // all inside
        let pt = layout.hit_test_point(Point::new(22.5, 0.0));
        assert_eq!(pt.idx, 4);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.idx, 4);
        let pt = layout.hit_test_point(Point::new(25.0, 0.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(26.0, 0.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(27.0, 0.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(28.0, 0.0));
        assert_eq!(pt.idx, 5);

        // outside
        println!("layout_width: {:?}", layout.size().width); // 56.0

        let pt = layout.hit_test_point(Point::new(56.0, 0.0));
        assert_eq!(pt.idx, 10); // last text position
        assert_eq!(pt.is_inside, true);

        let pt = layout.hit_test_point(Point::new(57.0, 0.0));
        assert_eq!(pt.idx, 10); // last text position
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(-1.0, 0.0));
        assert_eq!(pt.idx, 0); // first text position
        assert_eq!(pt.is_inside, false);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_hit_test_point_basic_0() {
        let mut text_layout = CairoText::new();

        let layout = text_layout.new_text_layout("piet text!").build().unwrap();
        println!("text pos 4: {:?}", layout.hit_test_text_position(4)); // 19.34765625
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 22.681640625

        // test hit test point
        // all inside
        let pt = layout.hit_test_point(Point::new(19.0, 0.0));
        assert_eq!(pt.idx, 4);
        let pt = layout.hit_test_point(Point::new(20.0, 0.0));
        assert_eq!(pt.idx, 4);
        let pt = layout.hit_test_point(Point::new(21.0, 0.0));
        assert_eq!(pt.idx, 4);
        let pt = layout.hit_test_point(Point::new(22.0, 0.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.idx, 5);

        // outside
        println!("layout_width: {:?}", layout.size().width); //45.357421875

        let pt = layout.hit_test_point(Point::new(45.0, 0.0));
        assert_eq!(pt.idx, 10); // last text position
        assert_eq!(pt.is_inside, true);

        let pt = layout.hit_test_point(Point::new(46.0, 0.0));
        assert_eq!(pt.idx, 10); // last text position
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(-1.0, 0.0));
        assert_eq!(pt.idx, 0); // first text position
        assert_eq!(pt.is_inside, false);
    }

    #[test]
    #[cfg(target_os = "linux")]
    // for testing that 'middle' assignment in binary search is correct
    fn test_hit_test_point_basic_1() {
        let mut text_layout = CairoText::new();

        // base condition, one grapheme
        let layout = text_layout.new_text_layout("t").build().unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0

        // two graphemes (to check that middle moves)
        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.idx, 0);

        let layout = text_layout.new_text_layout("te").build().unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 12.0

        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.idx, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.idx, 1);
        let pt = layout.hit_test_point(Point::new(6.0, 0.0));
        assert_eq!(pt.idx, 1);
        let pt = layout.hit_test_point(Point::new(11.0, 0.0));
        assert_eq!(pt.idx, 2);
    }

    #[test]
    #[cfg(target_os = "macos")]
    // for testing that 'middle' assignment in binary search is correct
    fn test_hit_test_point_basic_1() {
        let mut text_layout = CairoText::new();

        // base condition, one grapheme
        let layout = text_layout.new_text_layout("t").build().unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0

        // two graphemes (to check that middle moves)
        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.idx, 0);

        let layout = text_layout.new_text_layout("te").build().unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 12.0

        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.idx, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.idx, 1);
        let pt = layout.hit_test_point(Point::new(6.0, 0.0));
        assert_eq!(pt.idx, 1);
        let pt = layout.hit_test_point(Point::new(11.0, 0.0));
        assert_eq!(pt.idx, 2);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_hit_test_point_complex_0() {
        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇

        let mut text_layout = CairoText::new();
        let layout = text_layout.new_text_layout(input).build().unwrap();
        //println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 6.99999999
        //println!("text pos 9: {:?}", layout.hit_test_text_position(9)); // 24.0
        //println!("text pos 10: {:?}", layout.hit_test_text_position(10)); // 32.0
        //println!("text pos 14: {:?}", layout.hit_test_text_position(14)); // 39.0, line width

        let pt = layout.hit_test_point(Point::new(2.0, 0.0));
        assert_eq!(pt.idx, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(7.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(10.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(14.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(18.0, 0.0));
        assert_eq!(pt.idx, 9);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.idx, 9);
        let pt = layout.hit_test_point(Point::new(26.0, 0.0));
        assert_eq!(pt.idx, 9);
        let pt = layout.hit_test_point(Point::new(29.0, 0.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(32.0, 0.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(35.5, 0.0));
        assert_eq!(pt.idx, 14);
        let pt = layout.hit_test_point(Point::new(38.0, 0.0));
        assert_eq!(pt.idx, 14);
        let pt = layout.hit_test_point(Point::new(40.0, 0.0));
        assert_eq!(pt.idx, 14);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_hit_test_point_complex_0() {
        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇

        let mut text_layout = CairoText::new();
        let layout = text_layout.new_text_layout(input).build().unwrap();
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 6.673828125
        println!("text pos 9: {:?}", layout.hit_test_text_position(9)); // 28.55859375
        println!("text pos 10: {:?}", layout.hit_test_text_position(10)); // 35.232421875
        println!("text pos 14: {:?}", layout.hit_test_text_position(14)); // 42.8378905, line width

        let pt = layout.hit_test_point(Point::new(2.0, 0.0));
        assert_eq!(pt.idx, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(7.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(10.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(14.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(18.0, 0.0));
        assert_eq!(pt.idx, 9);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.idx, 9);
        let pt = layout.hit_test_point(Point::new(26.0, 0.0));
        assert_eq!(pt.idx, 9);
        let pt = layout.hit_test_point(Point::new(29.0, 0.0));
        assert_eq!(pt.idx, 9);
        let pt = layout.hit_test_point(Point::new(32.0, 0.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(35.5, 0.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(38.0, 0.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(40.0, 0.0));
        assert_eq!(pt.idx, 14);
        let pt = layout.hit_test_point(Point::new(43.0, 0.0));
        assert_eq!(pt.idx, 14);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_hit_test_point_complex_1() {
        // this input caused an infinite loop in the binary search when test position
        // > 21.0 && < 28.0
        //
        // This corresponds to the char 'y' in the input.
        let input = "tßßypi";

        let mut text_layout = CairoText::new();
        let layout = text_layout.new_text_layout(input).build().unwrap();
        println!("text pos 0: {:?}", layout.hit_test_text_position(0)); // 0.0
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0
        println!("text pos 3: {:?}", layout.hit_test_text_position(3)); // 13.0
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 21.0
        println!("text pos 6: {:?}", layout.hit_test_text_position(6)); // 28.0
        println!("text pos 7: {:?}", layout.hit_test_text_position(7)); // 36.0
        println!("text pos 8: {:?}", layout.hit_test_text_position(8)); // 39.0, end

        let pt = layout.hit_test_point(Point::new(27.0, 0.0));
        assert_eq!(pt.idx, 6);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_hit_test_point_complex_1() {
        // this input caused an infinite loop in the binary search when test position
        // > 21.0 && < 28.0
        //
        // This corresponds to the char 'y' in the input.
        let input = "tßßypi";

        let mut text_layout = CairoText::new();
        let layout = text_layout.new_text_layout(input).build().unwrap();
        println!("text pos 0: {:?}", layout.hit_test_text_position(0)); // 0.0
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0
        println!("text pos 3: {:?}", layout.hit_test_text_position(3)); // 13.0
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 21.0
        println!("text pos 6: {:?}", layout.hit_test_text_position(6)); // 28.0
        println!("text pos 7: {:?}", layout.hit_test_text_position(7)); // 36.0
        println!("text pos 8: {:?}", layout.hit_test_text_position(8)); // 39.0, end

        let pt = layout.hit_test_point(Point::new(27.0, 0.0));
        assert_eq!(pt.idx, 6);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_multiline_hit_test_text_position_basic() {
        let mut text_layout = CairoText::new();

        let input = "piet  text!";

        let layout = text_layout.new_text_layout(&input[0..3]).build().unwrap();
        let pie_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[0..4])
            .max_width(25.0)
            .build()
            .unwrap();
        let piet_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[0..5])
            .max_width(30.)
            .build()
            .unwrap();
        let piet_space_width = layout.size().width;

        // "text" should be on second line
        let layout = text_layout
            .new_text_layout(&input[6..10])
            .max_width(25.0)
            .build()
            .unwrap();
        let text_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[6..9])
            .max_width(25.0)
            .build()
            .unwrap();
        let tex_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[6..8])
            .max_width(25.0)
            .build()
            .unwrap();
        let te_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[6..7])
            .max_width(25.0)
            .build()
            .unwrap();
        let t_width = layout.size().width;

        let full_layout = text_layout
            .new_text_layout(input)
            .max_width(25.0)
            .build()
            .unwrap();

        println!("lm: {:#?}", full_layout.line_metrics);
        println!("layout width: {:#?}", full_layout.size().width);

        println!("'pie': {}", pie_width);
        println!("'piet': {}", piet_width);
        println!("'piet ': {}", piet_space_width);
        println!("'text': {}", text_width);
        println!("'tex': {}", tex_width);
        println!("'te': {}", te_width);
        println!("'t': {}", t_width);

        // NOTE these heights are representative of baseline-to-baseline measures
        let line_zero_baseline = full_layout
            .line_metric(0)
            .map(|l| l.y_offset + l.baseline)
            .unwrap();
        let line_one_baseline = full_layout
            .line_metric(1)
            .map(|l| l.y_offset + l.baseline)
            .unwrap();

        // these just test the x position of text positions on the second line
        assert_close!(
            full_layout.hit_test_text_position(10).point.x,
            text_width,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(9).point.x,
            tex_width,
            3.0,
        );
        assert_close!(full_layout.hit_test_text_position(8).point.x, te_width, 3.0,);
        assert_close!(full_layout.hit_test_text_position(7).point.x, t_width, 3.0,);
        // This should be beginning of second line
        assert_close!(full_layout.hit_test_text_position(6).point.x, 0.0, 3.0,);

        assert_close!(
            full_layout.hit_test_text_position(3).point.x,
            pie_width,
            3.0,
        );

        // These test y position of text positions on line 1 (0-index)
        assert_close!(
            full_layout.hit_test_text_position(10).point.y,
            line_one_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(9).point.y,
            line_one_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(8).point.y,
            line_one_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(7).point.y,
            line_one_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(6).point.y,
            line_one_baseline,
            3.0,
        );

        // this tests y position of 0 line
        assert_close!(
            full_layout.hit_test_text_position(5).point.y,
            line_zero_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(4).point.y,
            line_zero_baseline,
            3.0,
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_multiline_hit_test_text_position_basic() {
        let mut text_layout = CairoText::new();

        let input = "piet  text!";
        let font = text_layout
            .font_family("Helvetica") // change this for osx
            .unwrap();

        let layout = text_layout
            .new_text_layout(&input[0..3])
            .font(font.clone(), 15.0)
            .max_width(30.0)
            .build()
            .unwrap();
        let pie_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[0..4])
            .font(font.clone(), 15.0)
            .max_width(25.0)
            .build()
            .unwrap();
        let piet_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[0..5])
            .font(font.clone(), 15.0)
            .max_width(30.0)
            .build()
            .unwrap();
        let piet_space_width = layout.size().width;

        // "text" should be on second line
        let layout = text_layout
            .new_text_layout(&input[6..10])
            .font(font.clone(), 15.0)
            .max_width(25.0)
            .build()
            .unwrap();
        let text_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[6..9])
            .font(font.clone(), 15.0)
            .max_width(25.0)
            .build()
            .unwrap();
        let tex_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[6..8])
            .font(font.clone(), 15.0)
            .max_width(25.0)
            .build()
            .unwrap();
        let te_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[6..7])
            .font(font.clone(), 15.0)
            .max_width(25.0)
            .build()
            .unwrap();
        let t_width = layout.size().width;

        let full_layout = text_layout
            .new_text_layout(input)
            .font(font, 15.0)
            .max_width(25.0)
            .build()
            .unwrap();

        println!("lm: {:#?}", full_layout.line_metrics);
        println!("layout width: {:#?}", full_layout.size().width);

        println!("'pie': {}", pie_width);
        println!("'piet': {}", piet_width);
        println!("'piet ': {}", piet_space_width);
        println!("'text': {}", text_width);
        println!("'tex': {}", tex_width);
        println!("'te': {}", te_width);
        println!("'t': {}", t_width);

        // NOTE these heights are representative of baseline-to-baseline measures
        let line_zero_baseline = full_layout
            .line_metric(0)
            .map(|l| l.y_offset + l.baseline)
            .unwrap();
        let line_one_baseline = full_layout
            .line_metric(1)
            .map(|l| l.y_offset + l.baseline)
            .unwrap();

        // these just test the x position of text positions on the second line
        assert_close!(
            full_layout.hit_test_text_position(10).point.x,
            text_width,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(9).point.x,
            tex_width,
            3.0,
        );
        assert_close!(full_layout.hit_test_text_position(8).point.x, te_width, 3.0,);
        assert_close!(full_layout.hit_test_text_position(7).point.x, t_width, 3.0,);
        // This should be beginning of second line
        assert_close!(full_layout.hit_test_text_position(6).point.x, 0.0, 3.0,);

        assert_close!(
            full_layout.hit_test_text_position(3).point.x,
            pie_width,
            3.0,
        );

        // These test y position of text positions on line 1 (0-index)
        assert_close!(
            full_layout.hit_test_text_position(10).point.y,
            line_one_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(9).point.y,
            line_one_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(8).point.y,
            line_one_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(7).point.y,
            line_one_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(6).point.y,
            line_one_baseline,
            3.0,
        );

        // this tests y position of 0 line
        assert_close!(
            full_layout.hit_test_text_position(5).point.y,
            line_zero_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(4).point.y,
            line_zero_baseline,
            3.0,
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    // very basic testing that multiline works
    fn test_multiline_hit_test_point_basic() {
        let input = "piet text most best";
        let mut text = CairoText::new();

        // this should break into four lines
        let layout = text.new_text_layout(input).max_width(30.0).build().unwrap();
        println!("text pos 01: {:?}", layout.hit_test_text_position(0)); // (0.0, 12.0)
        println!("text pos 06: {:?}", layout.hit_test_text_position(5)); // (0.0, 26.0)
        println!("text pos 11: {:?}", layout.hit_test_text_position(10)); // (0.0, 40.0)
        println!("text pos 16: {:?}", layout.hit_test_text_position(15)); // (0.0, 53.99999)

        let pt = layout.hit_test_point(Point::new(1.0, -1.0));
        assert_eq!(pt.idx, 0);
        assert_eq!(pt.is_inside, false);
        let pt = layout.hit_test_point(Point::new(1.0, 00.0));
        assert_eq!(pt.idx, 0);
        assert!(pt.is_inside);
        let pt = layout.hit_test_point(Point::new(1.0, 14.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(1.0, 28.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(1.0, 44.0));
        assert_eq!(pt.idx, 15);

        // over on y axis, but x still affects the text position
        let best_layout = text.new_text_layout("best").build().unwrap();
        println!("layout width: {:#?}", best_layout.size().width); // 26.0...

        let pt = layout.hit_test_point(Point::new(1.0, 56.0));
        assert_eq!(pt.idx, 15);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(25.0, 56.0));
        assert_eq!(pt.idx, 19);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(27.0, 56.0));
        assert_eq!(pt.idx, 19);
        assert_eq!(pt.is_inside, false);

        // under
        let piet_layout = text.new_text_layout("piet ").build().unwrap();
        println!("layout width: {:#?}", piet_layout.size().width); // 27.0...

        let pt = layout.hit_test_point(Point::new(1.0, -14.0)); // under
        assert_eq!(pt.idx, 0);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(26.0, -14.0)); // under
        assert_eq!(pt.idx, 5);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(28.0, -14.0)); // under
        assert_eq!(pt.idx, 5);
        assert_eq!(pt.is_inside, false);
    }

    #[test]
    #[cfg(target_os = "macos")]
    // very basic testing that multiline works
    fn test_multiline_hit_test_point_basic() {
        let input = "piet text most best";
        let mut text = CairoText::new();

        let font = text.font_family("Helvetica").unwrap();
        // this should break into four lines
        let layout = text
            .new_text_layout(input)
            .font(font.clone(), 13.0)
            .max_width(30.0)
            .build()
            .unwrap();
        println!("text pos 01: {:?}", layout.hit_test_text_position(0)); // (0.0, 0.0)
        println!("text pos 06: {:?}", layout.hit_test_text_position(5)); // (0.0, 13.0)
        println!("text pos 11: {:?}", layout.hit_test_text_position(10)); // (0.0, 26.0)
        println!("text pos 16: {:?}", layout.hit_test_text_position(15)); // (0.0, 39.0)

        let pt = layout.hit_test_point(Point::new(1.0, -1.0));
        assert_eq!(pt.idx, 0);
        assert_eq!(pt.is_inside, false);
        let pt = layout.hit_test_point(Point::new(1.0, 00.0));
        assert_eq!(pt.idx, 0);
        assert!(pt.is_inside);
        let pt = layout.hit_test_point(Point::new(1.0, 12.));
        assert_eq!(pt.idx, 0);
        let pt = layout.hit_test_point(Point::new(1.0, 13.));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(1.0, 26.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(1.0, 39.0));
        assert_eq!(pt.idx, 15);
        assert!(pt.is_inside);

        // over on y axis, but x still affects the text position
        let best_layout = text
            .new_text_layout("best")
            .font(font.clone(), 13.0)
            .build()
            .unwrap();
        println!("layout width: {:#?}", best_layout.size().width); // 26.0...

        let pt = layout.hit_test_point(Point::new(1.0, 52.0));
        assert_eq!(pt.idx, 15);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(25.0, 52.0));
        assert_eq!(pt.idx, 19);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(27.0, 52.0));
        assert_eq!(pt.idx, 19);
        assert_eq!(pt.is_inside, false);

        // under
        let piet_layout = text
            .new_text_layout("piet ")
            .font(font, 13.0)
            .build()
            .unwrap();
        println!("layout width: {:#?}", piet_layout.size().width); // ???

        let pt = layout.hit_test_point(Point::new(1.0, -14.0)); // under
        assert_eq!(pt.idx, 0);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(25.0, -14.0)); // under
        assert_eq!(pt.idx, 5);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(27.0, -14.0)); // under
        assert_eq!(pt.idx, 5);
        assert_eq!(pt.is_inside, false);
    }
}
