//! Text functionality for Piet cairo backend

use std::convert::TryInto;
use std::fmt;
use std::ops::{Range, RangeBounds};
use std::rc::Rc;

use glib::translate::{from_glib_full, ToGlibPtr};

use pango::prelude::FontFamilyExt;
use pango::prelude::FontMapExt;
use pango::AttrList;
use pango_sys::pango_attr_insert_hyphens_new;
use pangocairo::FontMap;

use piet::kurbo::{Point, Rect, Size, Vec2};
use piet::{
    util, Error, FontFamily, FontStyle, HitTestPoint, HitTestPosition, LineMetric, Text,
    TextAlignment, TextAttribute, TextLayout, TextLayoutBuilder, TextStorage,
};

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
    is_rtl: bool,
    size: Size,
    ink_rect: Rect,
    pango_offset: Vec2,
    trailing_ws_width: f64,

    line_metrics: Rc<[LineMetric]>,
    x_offsets: Rc<[i32]>,
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
    fn into_pango(self) -> PangoAttribute {
        let mut pango_attribute = match &self.attribute {
            TextAttribute::FontFamily(family) => {
                let family = family.name();
                /*
                 * NOTE: If the family fails to resolve we just don't apply the attribute.
                 * That allows Pango to use its default font of choice to render that text
                 */
                PangoAttribute::new_family(family)
            }

            TextAttribute::FontSize(size) => {
                let size = (size * PANGO_SCALE) as i32;
                PangoAttribute::new_size_absolute(size)
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

                PangoAttribute::new_weight(pango_weights[closest_index].1)
            }

            TextAttribute::TextColor(text_color) => {
                let (r, g, b, _) = text_color.as_rgba8();
                PangoAttribute::new_foreground(
                    (r as u16 * 256) + (r as u16),
                    (g as u16 * 256) + (g as u16),
                    (b as u16 * 256) + (b as u16),
                )
            }

            TextAttribute::Style(style) => {
                let style = match style {
                    FontStyle::Regular => PangoStyle::Normal,
                    FontStyle::Italic => PangoStyle::Italic,
                };
                PangoAttribute::new_style(style)
            }

            &TextAttribute::Underline(underline) => {
                let underline = if underline {
                    PangoUnderline::Single
                } else {
                    PangoUnderline::None
                };
                PangoAttribute::new_underline(underline)
            }

            &TextAttribute::Strikethrough(strikethrough) => {
                PangoAttribute::new_strikethrough(strikethrough)
            }
        };

        if let Some(range) = self.range {
            pango_attribute.set_start_index(range.start.try_into().unwrap());
            pango_attribute.set_end_index(range.end.try_into().unwrap());
        }

        pango_attribute
    }
}

impl CairoText {
    /// Create a new factory that satisfies the piet `Text` trait.
    #[allow(clippy::new_without_default)]
    pub fn new() -> CairoText {
        let fontmap = FontMap::default().unwrap();
        CairoText {
            pango_context: fontmap.create_context().unwrap(),
        }
    }
}

impl Text for CairoText {
    type TextLayout = CairoTextLayout;
    type TextLayoutBuilder = CairoTextLayoutBuilder;

    fn font_family(&mut self, family_name: &str) -> Option<FontFamily> {
        // The pango documentation says this is always a string, and never null.
        self.pango_context
            .list_families()
            .iter()
            .find(|family| family.name().unwrap().as_str() == family_name);
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

        if let Some(attr) = unsafe { from_glib_full(pango_attr_insert_hyphens_new(0)) } {
            pango_attributes.insert(attr);
        }

        pango_attributes.insert(
            AttributeWithRange {
                attribute: TextAttribute::FontFamily(self.defaults.font),
                range: None,
            }
            .into_pango(),
        );
        pango_attributes.insert(
            AttributeWithRange {
                attribute: TextAttribute::FontSize(self.defaults.font_size),
                range: None,
            }
            .into_pango(),
        );
        pango_attributes.insert(
            AttributeWithRange {
                attribute: TextAttribute::Weight(self.defaults.weight),
                range: None,
            }
            .into_pango(),
        );
        pango_attributes.insert(
            AttributeWithRange {
                attribute: TextAttribute::TextColor(self.defaults.fg_color),
                range: None,
            }
            .into_pango(),
        );
        pango_attributes.insert(
            AttributeWithRange {
                attribute: TextAttribute::Style(self.defaults.style),
                range: None,
            }
            .into_pango(),
        );
        pango_attributes.insert(
            AttributeWithRange {
                attribute: TextAttribute::Underline(self.defaults.underline),
                range: None,
            }
            .into_pango(),
        );
        pango_attributes.insert(
            AttributeWithRange {
                attribute: TextAttribute::Strikethrough(self.defaults.strikethrough),
                range: None,
            }
            .into_pango(),
        );

        for attribute in self.attributes {
            pango_attributes.insert(attribute.into_pango());
        }

        self.pango_layout.set_attributes(Some(&pango_attributes));
        self.pango_layout.set_wrap(pango::WrapMode::WordChar);
        self.pango_layout.set_ellipsize(pango::EllipsizeMode::None);

        // invalid until update_width() is called
        let mut layout = CairoTextLayout {
            is_rtl: util::first_strong_rtl(self.text.as_str()),
            text: self.text,
            size: Size::ZERO,
            ink_rect: Rect::ZERO,
            pango_offset: Vec2::ZERO,
            trailing_ws_width: 0.0,
            line_metrics: Rc::new([]),
            x_offsets: Rc::new([]),
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
        self.ink_rect
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
        let point = point + self.pango_offset;

        let line_number = self
            .line_metrics
            .iter()
            .position(|lm| lm.y_offset + lm.height >= point.y)
            // if we're past the last line, use the last line
            .unwrap_or_else(|| self.line_metrics.len().saturating_sub(1));
        let x_offset = self.x_offsets[line_number];
        let x = (point.x * PANGO_SCALE) as i32 - x_offset;

        let line = self
            .pango_layout
            .line(line_number.try_into().unwrap())
            .unwrap();

        let line_text = self.line_text(line_number).unwrap();
        let line_start_idx = self.line_metric(line_number).unwrap().start_offset;

        let hitpos = line.x_to_index(x as i32);
        let rel_idx = if hitpos.is_inside {
            let idx = hitpos.index as usize - line_start_idx;
            let trailing_len: usize = (&line_text[idx..])
                .chars()
                .take(hitpos.trailing as usize)
                .map(char::len_utf8)
                .sum();
            idx + trailing_len
        } else {
            let hit_is_left = x <= 0;
            let hard_break_len = match line_text.as_bytes() {
                [.., b'\r', b'\n'] => 2,
                [.., b'\n'] => 1,
                _ => 0,
            };
            if hit_is_left == self.is_rtl {
                line_text.len().saturating_sub(hard_break_len)
            } else {
                0
            }
        };

        let is_inside_y = point.y >= 0. && point.y <= self.size.height;

        HitTestPoint::new(line_start_idx + rel_idx, hitpos.is_inside && is_inside_y)
    }

    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition {
        let idx = idx.min(self.text.len());
        assert!(self.text.is_char_boundary(idx));

        let line_number = self
            .line_metrics
            .iter()
            .enumerate()
            .find(|(_, metric)| metric.start_offset <= idx && idx < metric.end_offset)
            .map(|(idx, _)| idx)
            .unwrap_or_else(|| self.line_metrics.len() - 1);
        let metric = self.line_metric(line_number).unwrap();

        // in RTL text, pango mishandles the very last position in the layout
        // https://gitlab.gnome.org/GNOME/pango/-/issues/544

        let hack_around_eol = self.is_rtl && idx == self.text.len();
        let idx = if hack_around_eol {
            // pango doesn't care if this is a char boundary
            idx.saturating_sub(1)
        } else {
            idx
        };

        let pos_rect = self.pango_layout.index_to_pos(idx as i32);
        let x = if hack_around_eol {
            pos_rect.x + pos_rect.width
        } else {
            pos_rect.x
        };

        let point = Point::new(
            (x as f64 / PANGO_SCALE) - self.pango_offset.x,
            (pos_rect.y as f64 / PANGO_SCALE) + metric.baseline - self.pango_offset.y,
        );

        HitTestPosition::new(point, line_number)
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
        let new_width = new_width
            .into()
            .map(|w| pango::SCALE.saturating_mul(w as i32))
            .unwrap_or(UNBOUNDED_WRAP_WIDTH);
        self.pango_layout.set_width(new_width);

        let mut line_metrics = Vec::new();
        let mut x_offsets = Vec::new();
        let mut y_offset = 0.;
        let mut widest_logical_width = 0;
        let mut widest_whitespaceless_width = 0;
        let mut iterator = self.pango_layout.iter().unwrap();
        loop {
            let line = iterator.line_readonly().unwrap();

            //FIXME: replace this when pango 0.10.0 lands
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

            let logical_rect = iterator.line_extents().1;
            if logical_rect.width > widest_logical_width {
                widest_logical_width = logical_rect.width;
            }

            let line_text = &self.text[start_offset..end_offset];
            let trimmed_len = line_text.trim_end().len();
            let trailing_whitespace = line_text[trimmed_len..].len();

            //HACK: This check for RTL is to work around https://gitlab.gnome.org/GNOME/pango/-/issues/544
            let non_ws_width = if trailing_whitespace != 0 && !self.is_rtl {
                //FIXME: this probably isn't correct for RTL
                line.index_to_x((start_offset + trimmed_len) as i32, false)
            } else {
                logical_rect.width
            };
            widest_whitespaceless_width = widest_whitespaceless_width.max(non_ws_width);

            x_offsets.push(logical_rect.x);
            line_metrics.push(LineMetric {
                start_offset,
                end_offset,
                trailing_whitespace,
                baseline: (iterator.baseline() as f64 / PANGO_SCALE) - y_offset,
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
        self.x_offsets = x_offsets.into();

        let (ink_extent, logical_extent) = self.pango_layout.extents();
        let ink_extent = to_kurbo_rect(ink_extent);
        let logical_extent = to_kurbo_rect(logical_extent);

        self.size = Size::new(
            widest_whitespaceless_width as f64 / PANGO_SCALE,
            logical_extent.height(),
        );

        self.ink_rect = ink_extent;
        self.pango_offset = logical_extent.origin().to_vec2();
        self.trailing_ws_width = widest_logical_width as f64 / PANGO_SCALE;
    }
}

fn to_kurbo_rect(r: pango::Rectangle) -> Rect {
    Rect::from_origin_size(
        (r.x as f64 / PANGO_SCALE, r.y as f64 / PANGO_SCALE),
        (r.width as f64 / PANGO_SCALE, r.height as f64 / PANGO_SCALE),
    )
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
}
