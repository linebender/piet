//! Text functionality for Piet cairo backend

use std::convert::TryInto;
use std::fmt;
use std::ops::{Range, RangeBounds};
use std::rc::Rc;

use glib::translate::{from_glib_full, ToGlibPtr};

use pango::{AttrList, FontMapExt};
use pango_sys::pango_attr_insert_hyphens_new;
use pangocairo::FontMap;

use piet::kurbo::{Point, Rect, Size, Vec2};
use piet::{
    util, Error, FontFamily, FontStyle, HitTestPoint, HitTestPosition, LineMetric, Text,
    TextAlignment, TextAttribute, TextLayout, TextLayoutBuilder, TextStorage,
};

use unicode_segmentation::GraphemeCursor;

type PangoLayout = pango::Layout;
type PangoContext = pango::Context;
type PangoAttribute = pango::Attribute;
type PangoWeight = pango::Weight;
type PangoStyle = pango::Style;
type PangoUnderline = pango::Underline;
type PangoAlignment = pango::Alignment;

const PANGO_SCALE: f64 = pango::SCALE as f64;
const UNBOUNDED_WRAP_WIDTH: i32 = -1;

#[derive(Clone)]
pub struct CairoText {
    pango_context: PangoContext,
}

#[derive(Clone)]
pub struct CairoTextLayout {
    text: Rc<dyn TextStorage>,

    size: Size,
    ink_size: Size,
    pango_offset: Vec2,
    trailing_ws_width: f64,

    line_metrics: Rc<[LineMetric]>,
    pango_layout: PangoLayout,
}

pub struct CairoTextLayoutBuilder {
    text: Rc<dyn TextStorage>,
    defaults: util::LayoutDefaults,
    attributes: Vec<AttributeWithRange>,
    last_range_start_pos: usize,
    width_constraint: f64,
    pango_layout: PangoLayout,
}

struct AttributeWithRange {
    attribute: TextAttribute,
    range: Option<Range<usize>>, //No range == entire layout
}

impl AttributeWithRange {
    fn into_pango(self) -> Option<PangoAttribute> {
        let mut pango_attribute = match &self.attribute {
            TextAttribute::FontFamily(family) => {
                let family = family.name();
                /*
                 * NOTE: If the family fails to resolve we just don't apply the attribute.
                 * That allows Pango to use its default font of choice to render that text
                 */
                PangoAttribute::new_family(family)?
            }

            TextAttribute::FontSize(size) => {
                let size = (size * PANGO_SCALE) as i32;
                PangoAttribute::new_size_absolute(size).unwrap()
            }

            TextAttribute::Weight(weight) => {
                //This is horrid
                let pango_weights = [
                    (100, PangoWeight::Thin),
                    (200, PangoWeight::Ultralight),
                    (300, PangoWeight::Light),
                    (350, PangoWeight::Semilight),
                    (380, PangoWeight::Book),
                    (400, PangoWeight::Normal),
                    (500, PangoWeight::Medium),
                    (600, PangoWeight::Semibold),
                    (700, PangoWeight::Bold),
                    (800, PangoWeight::Ultrabold),
                    (900, PangoWeight::Heavy),
                    (1_000, PangoWeight::Ultraheavy),
                ];

                let weight = weight.to_raw() as i32;
                let mut closest_index = 0;
                let mut closest_distance = 2_000; //Random very large value
                for (current_index, pango_weight) in pango_weights.iter().enumerate() {
                    let distance = (pango_weight.0 - weight).abs();
                    if distance < closest_distance {
                        closest_distance = distance;
                        closest_index = current_index;
                    }
                }

                PangoAttribute::new_weight(pango_weights[closest_index].1).unwrap()
            }

            TextAttribute::TextColor(text_color) => {
                let (r, g, b, _) = text_color.as_rgba8();
                PangoAttribute::new_foreground(
                    (r as u16 * 256) + (r as u16),
                    (g as u16 * 256) + (g as u16),
                    (b as u16 * 256) + (b as u16),
                )
                .unwrap()
            }

            TextAttribute::Style(style) => {
                let style = match style {
                    FontStyle::Regular => PangoStyle::Normal,
                    FontStyle::Italic => PangoStyle::Italic,
                };
                PangoAttribute::new_style(style).unwrap()
            }

            &TextAttribute::Underline(underline) => {
                let underline = if underline {
                    PangoUnderline::Single
                } else {
                    PangoUnderline::None
                };
                PangoAttribute::new_underline(underline).unwrap()
            }

            &TextAttribute::Strikethrough(strikethrough) => {
                PangoAttribute::new_strikethrough(strikethrough).unwrap()
            }
        };

        if let Some(range) = self.range {
            pango_attribute.set_start_index(range.start.try_into().unwrap());
            pango_attribute.set_end_index(range.end.try_into().unwrap());
        }

        Some(pango_attribute)
    }
}

impl CairoText {
    /// Create a new factory that satisfies the piet `Text` trait.
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
        //TODO: Veryify that a family exists with the requested name
        Some(FontFamily::new_unchecked(family_name))
    }

    fn load_font(&mut self, _data: &[u8]) -> Result<FontFamily, Error> {
        /*
         * NOTE(ForLoveOfCats): It does not appear that Pango natively supports loading font
         * data raw. All online resource I've seen so far point to registering fonts with
         * fontconfig and then letting Pango grab it from there but they all assume you have
         * a font file path which we do not have here.
         * See: https://gitlab.freedesktop.org/fontconfig/fontconfig/-/issues/12
         */
        Err(Error::NotSupported)
    }

    fn new_text_layout(&mut self, text: impl TextStorage) -> Self::TextLayoutBuilder {
        let pango_layout = PangoLayout::new(&self.pango_context);
        pango_layout.set_text(text.as_str());

        pango_layout.set_alignment(PangoAlignment::Left);
        pango_layout.set_justify(false);

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

impl TextLayoutBuilder for CairoTextLayoutBuilder {
    type Out = CairoTextLayout;

    fn max_width(mut self, width: f64) -> Self {
        self.width_constraint = width;
        self
    }

    fn alignment(self, alignment: TextAlignment) -> Self {
        /*
         * NOTE: Pango has `auto_dir` enabled by default. This means that
         * when it encounters a paragraph starting with a left-to-right
         * character the meanings of `Left` and `Right` are switched for
         * that paragraph. As a result the meaning of Piet's own `Start`
         * and `End` are preserved
         *
         * See: http://gtk-rs.org/docs/pango/struct.Layout.html#method.set_auto_dir
         */

        match alignment {
            TextAlignment::Start => {
                self.pango_layout.set_justify(false);
                self.pango_layout.set_alignment(PangoAlignment::Left);
            }

            TextAlignment::End => {
                self.pango_layout.set_justify(false);
                self.pango_layout.set_alignment(PangoAlignment::Right);
            }

            TextAlignment::Center => {
                self.pango_layout.set_justify(false);
                self.pango_layout.set_alignment(PangoAlignment::Center);
            }

            TextAlignment::Justified => {
                self.pango_layout.set_alignment(PangoAlignment::Left);
                self.pango_layout.set_justify(true);
            }
        }

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

        self.attributes.push(AttributeWithRange {
            attribute,
            range: Some(range),
        });

        self
    }

    fn build(self) -> Result<Self::Out, Error> {
        let pango_attributes = AttrList::new();
        let add_attribute = |attribute| {
            if let Some(attribute) = attribute {
                pango_attributes.insert(attribute);
            }
        };

        if let Some(attr) = unsafe { from_glib_full(pango_attr_insert_hyphens_new(0)) } {
            pango_attributes.insert(attr);
        }

        add_attribute(
            AttributeWithRange {
                attribute: TextAttribute::FontFamily(self.defaults.font),
                range: None,
            }
            .into_pango(),
        );
        add_attribute(
            AttributeWithRange {
                attribute: TextAttribute::FontSize(self.defaults.font_size),
                range: None,
            }
            .into_pango(),
        );
        add_attribute(
            AttributeWithRange {
                attribute: TextAttribute::Weight(self.defaults.weight),
                range: None,
            }
            .into_pango(),
        );
        add_attribute(
            AttributeWithRange {
                attribute: TextAttribute::TextColor(self.defaults.fg_color),
                range: None,
            }
            .into_pango(),
        );
        add_attribute(
            AttributeWithRange {
                attribute: TextAttribute::Style(self.defaults.style),
                range: None,
            }
            .into_pango(),
        );
        add_attribute(
            AttributeWithRange {
                attribute: TextAttribute::Underline(self.defaults.underline),
                range: None,
            }
            .into_pango(),
        );
        add_attribute(
            AttributeWithRange {
                attribute: TextAttribute::Strikethrough(self.defaults.strikethrough),
                range: None,
            }
            .into_pango(),
        );

        for attribute in self.attributes {
            add_attribute(attribute.into_pango());
        }

        self.pango_layout.set_attributes(Some(&pango_attributes));

        //NOTE: We give Pango a width of -1 in `update_width` when we don't want wrapping
        self.pango_layout.set_wrap(pango::WrapMode::WordChar);
        self.pango_layout.set_ellipsize(pango::EllipsizeMode::None);

        // invalid until update_width() is called
        let mut layout = CairoTextLayout {
            text: self.text,
            size: Size::ZERO,
            ink_size: Size::ZERO,
            pango_offset: Vec2::ZERO,
            trailing_ws_width: 0.0,
            line_metrics: Rc::new([]),
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
        self.ink_size.to_rect()
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
        let x = (point.x + self.pango_offset.x) * PANGO_SCALE;
        let y = (point.y + self.pango_offset.y) * PANGO_SCALE;

        let (is_inside, index, trailing) = self.pango_layout.xy_to_index(x as i32, y as i32);
        let index = if trailing == 0 {
            index.try_into().unwrap()
        } else {
            /*
             * NOTE(ForLoveOfCats): The docs specify that a non-zero value for trailing
             * indicates that the point aligns to the trailing edge of the grapheme. In
             * that case the value tells us the number of "characters" in the grapheme.
             */

            let text = &self.text;
            let index = index.try_into().unwrap();
            let trailing = trailing.try_into().unwrap();

            text[index..]
                .char_indices()
                .nth(trailing)
                .map(|(offset, _)| index + offset)
                .unwrap_or_else(|| text.len())
        };

        let (metric_index, metric) = self
            .line_metrics
            .iter()
            .enumerate()
            .find(|(_, metric)| metric.start_offset <= index && index < metric.end_offset)
            .unwrap_or_else(|| {
                /*
                 * NOTE: Handle out of bounds end of text index.
                 * An index such that `index == text.len()` is possible but the
                 * find predicate will not resolve to any metric in that case so
                 * always return the last metric if the find fails
                 */
                let index = self.line_metrics.len() - 1;
                (index, &self.line_metrics[index])
            });

        //NOTE: Manually move to start of next line when hit test is on a soft line break
        let index = {
            let text = &self.text;
            if index == text.len()
                || matches!(text.as_bytes()[index], b'\r' | b'\n')
                || metric_index + 1 >= self.line_metrics.len()
            {
                index
            } else {
                let mut iterator = GraphemeCursor::new(index, text.len(), true);
                let next = iterator
                    .next_boundary(text.as_str(), 0)
                    .unwrap_or(Some(index))
                    .unwrap_or(index);

                if next >= metric.end_offset {
                    next
                } else {
                    index
                }
            }
        };

        HitTestPoint::new(index, is_inside)
    }

    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition {
        let line = self
            .line_metrics
            .iter()
            .enumerate()
            .find_map(|(line_index, metric)| {
                if metric.start_offset <= idx && idx < metric.end_offset {
                    Some(line_index)
                } else {
                    None
                }
            })
            .unwrap_or(self.line_metrics.len() - 1);
        let metric = self.line_metric(line).unwrap();

        let pos_rect = self.pango_layout.index_to_pos(idx as i32);

        let point = Point::new(
            (pos_rect.x as f64 / PANGO_SCALE) - self.pango_offset.x,
            (pos_rect.y as f64 / PANGO_SCALE) + metric.baseline - self.pango_offset.y,
        );

        HitTestPosition::new(point, line)
    }
}

impl CairoTextLayout {
    pub(crate) fn pango_layout(&self) -> &PangoLayout {
        &self.pango_layout
    }

    pub(crate) fn pango_offset(&self) -> Vec2 {
        self.pango_offset
    }

    fn update_width(&mut self, new_width: impl Into<Option<f64>>) {
        if let Some(new_width) = new_width.into() {
            let pango_width = new_width * pango::SCALE as f64;
            self.pango_layout.set_width(pango_width as i32);
        } else {
            /*
             * NOTE: This is the default value, however `update_width` *could*
             * be called any number of times with different values so we need
             * to make sure to reset back to default whenever we get no width
             */
            self.pango_layout.set_width(UNBOUNDED_WRAP_WIDTH);
        }

        let mut line_metrics = Vec::new();
        let mut y_offset = 0.;
        let mut widest_logical_width = 0;
        let mut widest_whitespaceless_width = 0;
        let mut iterator = self.pango_layout.get_iter().unwrap();
        loop {
            let line = iterator.get_line_readonly().unwrap();

            /*
             * NOTE: These values are not currently exposed so we need to get them
             * manually. It's kinda sucky I know
             *
             * TODO(ForLoveOfCats): Submit a PR to gtk-rs to expose these values
             */
            let (start_offset, end_offset) = unsafe {
                let raw_line = line.to_glib_none();

                let start_offset = (*raw_line.0).start_index as usize;
                let length = (*raw_line.0).length as usize;

                (start_offset, start_offset + length)
            };

            //Pango likes to give us the line range *without* the newline char(s)
            let end_offset = match self.text.as_bytes()[end_offset..] {
                [b'\r', b'\n', ..] => end_offset + 2,
                [b'\r', ..] | [b'\n', ..] => end_offset + 1,
                _ => end_offset,
            };

            let logical_rect = line.get_extents().1;
            if logical_rect.width > widest_logical_width {
                widest_logical_width = logical_rect.width;
            }

            let line_text = &self.text[start_offset..end_offset];
            let trimmed_len = line_text.trim_end().len();
            let trailing_whitespace = line_text[trimmed_len..].len();

            widest_whitespaceless_width = {
                let start_x = line.index_to_x(trimmed_len as i32, false);
                let end_x = line.index_to_x(line_text.len() as i32, true);
                let whitespace_width = end_x - start_x;
                let whitespaceless_width = logical_rect.width - whitespace_width;
                widest_whitespaceless_width.max(whitespaceless_width)
            };

            line_metrics.push(LineMetric {
                start_offset,
                end_offset,
                trailing_whitespace,
                baseline: (iterator.get_baseline() as f64 / PANGO_SCALE) - y_offset,
                height: logical_rect.height as f64 / PANGO_SCALE,
                y_offset,
            });
            y_offset += logical_rect.height as f64 / PANGO_SCALE;

            if !iterator.next_line() {
                break;
            }
        }

        //NOTE: Pango appears to always give us at least one line even with empty input
        self.line_metrics = line_metrics.into();

        let ink_extent = self.pango_layout.get_extents().0;
        let logical_extent = self.pango_layout.get_extents().1;
        self.size = Size::new(
            widest_whitespaceless_width as f64 / PANGO_SCALE,
            logical_extent.height as f64 / PANGO_SCALE,
        );
        self.ink_size = Size::new(
            ink_extent.width as f64 / PANGO_SCALE,
            ink_extent.height as f64 / PANGO_SCALE,
        );
        self.pango_offset = Vec2::new(
            logical_extent.x as f64 / PANGO_SCALE,
            logical_extent.y as f64 / PANGO_SCALE,
        );

        self.trailing_ws_width = widest_logical_width as f64 / PANGO_SCALE;
    }
}

impl fmt::Debug for CairoTextLayout {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CairoTextLayout").finish()
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

        let pt = layout.hit_test_point(Point::new(55.0, 0.0));
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
        let pt = layout.hit_test_point(Point::new(38.0, 0.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(42.0, 0.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(46.5, 0.0));
        assert_eq!(pt.idx, 14);
        let pt = layout.hit_test_point(Point::new(52.0, 0.0));
        assert_eq!(pt.idx, 14);
        let pt = layout.hit_test_point(Point::new(58.0, 0.0));
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
        println!("text pos 06: {:?}", layout.hit_test_text_position(5)); // (0.0, 27.0)
        println!("text pos 11: {:?}", layout.hit_test_text_position(10)); // (0.0, 43.0)
        println!("text pos 16: {:?}", layout.hit_test_text_position(15)); // (0.0, 57.0)

        let pt = layout.hit_test_point(Point::new(1.0, -1.0));
        assert_eq!(pt.idx, 0);
        assert_eq!(pt.is_inside, false);
        let pt = layout.hit_test_point(Point::new(1.0, 00.0));
        assert_eq!(pt.idx, 0);
        assert!(pt.is_inside);
        let pt = layout.hit_test_point(Point::new(1.0, 16.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(1.0, 30.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(1.0, 56.0));
        assert_eq!(pt.idx, 15);

        // over on y axis, but x still affects the text position
        let best_layout = text.new_text_layout("best").build().unwrap();
        println!("layout width: {:#?}", best_layout.size().width); // 26.0...

        let pt = layout.hit_test_point(Point::new(1.0, 62.0));
        assert_eq!(pt.idx, 15);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(25.0, 62.0));
        assert_eq!(pt.idx, 19);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(27.0, 62.0));
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
