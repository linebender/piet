//! Text functionality for Piet direct2d backend

mod lines;

use std::cell::{Cell, RefCell};
use std::convert::TryInto;
use std::ops::{Range, RangeBounds};
use std::rc::Rc;
use std::sync::Arc;

pub use d2d::{D2DDevice, D2DFactory, DeviceContext as D2DDeviceContext};
pub use dwrite::DwriteFactory;
use dwrote::{CustomFontCollectionLoaderImpl, FontCollection, FontFile};
use winapi::um::d2d1::D2D1_DRAW_TEXT_OPTIONS_NONE;
use wio::wide::ToWide;

use piet::kurbo::{Insets, Point, Rect, Size};
use piet::util;
use piet::{
    Color, Error, FontFamily, HitTestPoint, HitTestPosition, LineMetric, RenderContext, Text,
    TextAlignment, TextAttribute, TextLayout, TextLayoutBuilder, TextStorage,
};

use crate::conv;
use crate::d2d;
use crate::dwrite::{self, TextFormat, Utf16Range};
use crate::D2DRenderContext;

#[derive(Clone)]
pub struct D2DText {
    dwrite: DwriteFactory,
    loaded_fonts: Rc<RefCell<LoadedFonts>>,
}

#[derive(Default)]
struct LoadedFonts {
    files: Vec<FontFile>,
    // - multiple files can have the same family name, so we don't want this to be a set.
    // - we assume a small number of custom fonts will be loaded; if that isn't true we
    // should use a set or something.
    names: Vec<FontFamily>,
    collection: Option<FontCollection>,
}

#[derive(Clone)]
pub struct D2DTextLayout {
    text: Rc<dyn TextStorage>,
    // currently calculated on build
    line_metrics: Rc<[LineMetric]>,
    size: Size,
    /// insets that, when applied to our layout rect, generates our inking/image rect.
    inking_insets: Insets,
    // this is in a refcell because we need to mutate it to set colors on first draw
    layout: Rc<RefCell<dwrite::TextLayout>>,
    // these two are used when the layout is empty, so we can still correctly
    // draw the cursor
    default_line_height: f64,
    default_baseline: f64,
    // colors are only added to the layout lazily, because we need access to d2d::DeviceContext
    // in order to generate the brushes.
    colors: Rc<[(Utf16Range, Color)]>,
    needs_to_set_colors: Cell<bool>,
}

pub struct D2DTextLayoutBuilder {
    text: Rc<dyn TextStorage>,
    layout: Result<dwrite::TextLayout, Error>,
    len_utf16: usize,
    loaded_fonts: Rc<RefCell<LoadedFonts>>,
    default_font: FontFamily,
    default_font_size: f64,
    colors: Vec<(Utf16Range, Color)>,
    // just used to assert api is used as expected
    last_range_start_pos: usize,
}

impl D2DText {
    /// Create a new factory that satisfies the piet `Text` trait given
    /// the (platform-specific) dwrite factory.
    pub fn new(dwrite: DwriteFactory) -> D2DText {
        D2DText {
            dwrite,
            loaded_fonts: Default::default(),
        }
    }

    #[cfg(test)]
    pub fn new_for_test() -> D2DText {
        let dwrite = DwriteFactory::new().unwrap();
        D2DText::new(dwrite)
    }
}

impl Text for D2DText {
    type TextLayoutBuilder = D2DTextLayoutBuilder;
    type TextLayout = D2DTextLayout;

    fn font_family(&mut self, family_name: &str) -> Option<FontFamily> {
        self.dwrite
            .system_font_collection()
            .ok()
            .and_then(|fonts| fonts.font_family(family_name))
    }

    fn load_font(&mut self, data: &[u8]) -> Result<FontFamily, Error> {
        self.loaded_fonts.borrow_mut().add(data)
    }

    fn new_text_layout(&mut self, text: impl TextStorage) -> Self::TextLayoutBuilder {
        let text = Rc::new(text);
        let width = f32::INFINITY;
        let wide_str = ToWide::to_wide(&text.as_str());
        let layout = TextFormat::new(&self.dwrite, &[], util::DEFAULT_FONT_SIZE as f32)
            .and_then(|format| dwrite::TextLayout::new(&self.dwrite, format, width, &wide_str))
            .map_err(Into::into);

        D2DTextLayoutBuilder {
            layout,
            text,
            len_utf16: wide_str.len(),
            colors: Vec::new(),
            loaded_fonts: self.loaded_fonts.clone(),
            default_font: FontFamily::default(),
            default_font_size: piet::util::DEFAULT_FONT_SIZE,
            last_range_start_pos: 0,
        }
    }
}

impl TextLayoutBuilder for D2DTextLayoutBuilder {
    type Out = D2DTextLayout;

    fn max_width(mut self, width: f64) -> Self {
        let result = match self.layout.as_mut() {
            Ok(layout) => layout.set_max_width(width),
            Err(_) => Ok(()),
        };
        if let Err(err) = result {
            self.layout = Err(err.into());
        }
        self
    }

    fn alignment(mut self, alignment: TextAlignment) -> Self {
        if let Ok(layout) = self.layout.as_mut() {
            layout.set_alignment(alignment);
        }
        self
    }

    fn default_attribute(mut self, attribute: impl Into<TextAttribute>) -> Self {
        debug_assert!(
            self.last_range_start_pos == 0,
            "default attributes must be added before range attributes"
        );
        let attribute = attribute.into();
        match &attribute {
            TextAttribute::FontFamily(font) => self.default_font = font.clone(),
            TextAttribute::FontSize(size) => self.default_font_size = *size,
            _ => (),
        }
        self.add_attribute_shared(attribute, None);
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
        self.add_attribute_shared(attribute, Some(range));
        self
    }

    fn build(self) -> Result<Self::Out, Error> {
        let (default_line_height, default_baseline) = self.get_default_line_height_and_baseline();
        let layout = self.layout?;

        let mut layout = D2DTextLayout {
            text: self.text,
            colors: self.colors.into(),
            needs_to_set_colors: Cell::new(true),
            line_metrics: Rc::new([]),
            layout: Rc::new(RefCell::new(layout)),
            size: Size::ZERO,
            inking_insets: Insets::ZERO,
            default_line_height,
            default_baseline,
        };
        layout.rebuild_metrics();
        Ok(layout)
    }
}

impl D2DTextLayoutBuilder {
    /// used for both range and default attributes
    fn add_attribute_shared(&mut self, attr: TextAttribute, range: Option<Range<usize>>) {
        if let Ok(layout) = self.layout.as_mut() {
            let utf16_range = match range {
                Some(range) => {
                    let start = util::count_utf16(&self.text[..range.start]);
                    let len = if range.end == self.text.len() {
                        self.len_utf16
                    } else {
                        util::count_utf16(&self.text[range])
                    };
                    Utf16Range::new(start, len)
                }
                None => Utf16Range::new(0, self.len_utf16),
            };

            match attr {
                TextAttribute::FontFamily(font) => {
                    let is_custom = self.loaded_fonts.borrow().contains(&font);
                    if is_custom {
                        let mut loaded = self.loaded_fonts.borrow_mut();
                        layout.set_font_collection(utf16_range, loaded.collection());
                    } else if !self.loaded_fonts.borrow().is_empty() {
                        // if we are using custom fonts we also need to set the collection
                        // back to the system collection explicity as needed
                        layout.set_font_collection(utf16_range, &FontCollection::system());
                    }
                    let family_name = resolve_family_name(&font);
                    layout.set_font_family(utf16_range, family_name);
                }
                TextAttribute::FontSize(size) => layout.set_size(utf16_range, size as f32),
                TextAttribute::Weight(weight) => layout.set_weight(utf16_range, weight),
                TextAttribute::Style(style) => layout.set_style(utf16_range, style),
                TextAttribute::Underline(flag) => layout.set_underline(utf16_range, flag),
                TextAttribute::Strikethrough(flag) => layout.set_strikethrough(utf16_range, flag),
                TextAttribute::ForegroundColor(color) => self.colors.push((utf16_range, color)),
            }
        }
    }

    fn get_default_line_height_and_baseline(&self) -> (f64, f64) {
        let family_name = resolve_family_name(&self.default_font);
        let is_custom = self.loaded_fonts.borrow().contains(&self.default_font);
        let family = if is_custom {
            let mut loaded = self.loaded_fonts.borrow_mut();
            loaded.collection().get_font_family_by_name(&family_name)
        } else {
            FontCollection::system().get_font_family_by_name(&family_name)
        };

        let family = match family {
            Some(family) => family,
            // absolute fallback; use font size as line height
            None => return (self.default_font_size, self.default_font_size * 0.8),
        };

        let font = family.get_first_matching_font(
            dwrote::FontWeight::Regular,
            dwrote::FontStretch::Normal,
            dwrote::FontStyle::Normal,
        );
        let metrics = font.metrics().metrics0();
        let ascent = metrics.ascent as f64;
        let vert_metrics = ascent + metrics.descent as f64 + metrics.lineGap as f64;
        let vert_fraction = vert_metrics / metrics.designUnitsPerEm as f64;
        let ascent_fraction = ascent / metrics.designUnitsPerEm as f64;

        let line_height = self.default_font_size * vert_fraction;
        let baseline = self.default_font_size * ascent_fraction;

        (line_height, baseline)
    }
}

impl TextLayout for D2DTextLayout {
    fn size(&self) -> Size {
        self.size
    }

    fn image_bounds(&self) -> Rect {
        self.size.to_rect() + self.inking_insets
    }

    fn text(&self) -> &str {
        &self.text
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        self.line_metrics
            .get(line_number)
            .map(|lm| &self.text[lm.start_offset..(lm.end_offset - lm.trailing_whitespace)])
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        if line_number == 0 && self.text.is_empty() {
            Some(LineMetric {
                baseline: self.default_baseline,
                height: self.default_line_height,
                ..Default::default()
            })
        } else {
            self.line_metrics.get(line_number).cloned()
        }
    }

    fn line_count(&self) -> usize {
        self.line_metrics.len()
    }

    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        // lossy from f64 to f32, but shouldn't have too much impact
        let htp = self
            .layout
            .borrow()
            .hit_test_point(point.x as f32, point.y as f32);

        // Round up to next grapheme cluster boundary if DirectWrite
        // reports a trailing hit.
        let text_position_16 = if htp.is_trailing_hit {
            htp.metrics.text_position + htp.metrics.length
        } else {
            htp.metrics.text_position
        } as usize;

        // Convert text position from utf-16 code units to utf-8 code units.
        let text_position = util::count_until_utf16(&self.text, text_position_16)
            .unwrap_or_else(|| self.text.len());

        HitTestPoint::new(text_position, htp.is_inside)
    }

    // Can panic if text position is not at a code point boundary, or if it's out of bounds.
    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition {
        let idx = idx.min(self.text.len());
        assert!(self.text.is_char_boundary(idx));

        if self.text.is_empty() {
            return HitTestPosition::new(Point::new(0., self.default_baseline), 0);
        }
        // Note: DirectWrite will just return the line width if text position is
        // out of bounds. This is what want for piet; return line width for the last text position
        // (equal to line.len()). This is basically returning line width for the last cursor
        // position.

        let trailing = false;
        let idx_16 = util::count_utf16(&self.text[..idx]);
        let line = util::line_number_for_position(&self.line_metrics, idx);
        // Maximum string length on Windows is 32bits; nothing we can do here.
        let idx_16: u32 = idx_16.try_into().unwrap();

        let mut hit_point = self
            .layout
            .borrow()
            .hit_test_text_position(idx_16, trailing)
            .map(|hit| Point::new(hit.point_x as f64, hit.point_y as f64))
            // if DWrite fails we just return 0, 0
            .unwrap_or_default();
        // Raw reported point is top of glyph run box; move to baseline.
        if let Some(metric) = self.line_metrics.get(line) {
            hit_point.y = metric.y_offset + metric.baseline;
        }
        HitTestPosition::new(hit_point, line)
    }
}

impl D2DTextLayout {
    // must be called after build and after updating the width
    fn rebuild_metrics(&mut self) {
        let line_metrics = lines::fetch_line_metrics(&self.text, &self.layout.borrow());
        let text_metrics = self.layout.borrow().get_metrics();
        let overhang = self.layout.borrow().get_overhang_metrics();

        let size = Size::new(text_metrics.width as f64, text_metrics.height as f64);
        let overhang_width = text_metrics.layoutWidth as f64 + overhang.x1;
        let overhang_height = text_metrics.layoutHeight as f64 + overhang.y1;

        let inking_insets = Insets::new(
            overhang.x0,
            overhang.y0,
            overhang_width - size.width,
            overhang_height - size.height,
        );

        self.size = size;
        self.line_metrics = line_metrics.into();
        self.inking_insets = inking_insets;
    }

    pub fn draw(&self, pos: Point, ctx: &mut D2DRenderContext) {
        if !self.text.is_empty() {
            self.resolve_colors_if_needed(ctx);
            let pos = conv::to_point2f(pos);
            let black_brush = ctx.solid_brush(Color::BLACK);
            let text_options = D2D1_DRAW_TEXT_OPTIONS_NONE;
            ctx.rt
                .draw_text_layout(pos, &self.layout.borrow(), &black_brush, text_options);
        }
    }

    fn resolve_colors_if_needed(&self, ctx: &mut D2DRenderContext) {
        if self.needs_to_set_colors.replace(false) {
            for (range, color) in self.colors.as_ref() {
                let brush = ctx.solid_brush(color.clone());
                self.layout.borrow_mut().set_foregound_brush(*range, brush)
            }
        }
    }
}

//  this is not especially robust, but all of these are preinstalled on win 7+
fn resolve_family_name(family: &FontFamily) -> &str {
    match family {
        f if f == &FontFamily::SYSTEM_UI || f == &FontFamily::SANS_SERIF => "Segoe UI",
        f if f == &FontFamily::SERIF => "Times New Roman",
        f if f == &FontFamily::MONOSPACE => "Consolas",
        other => other.name(),
    }
}

impl LoadedFonts {
    fn add(&mut self, font_data: &[u8]) -> Result<FontFamily, Error> {
        let font_data: Arc<Vec<u8>> = Arc::new(font_data.to_owned());
        let font_file = FontFile::new_from_data(font_data).ok_or(Error::FontLoadingFailed)?;
        let collection_loader = CustomFontCollectionLoaderImpl::new(&[font_file.clone()]);
        let collection = FontCollection::from_loader(collection_loader);
        let mut families = collection.families_iter();
        let first_fam_name = families
            .next()
            .map(|f| f.name())
            .ok_or(Error::FontLoadingFailed)?;
        // just being defensive:
        if families.any(|f| f.name() != first_fam_name) {
            eprintln!("loaded font contains multiple family names");
        }

        let fam_name = FontFamily::new_unchecked(first_fam_name);
        self.files.push(font_file);
        self.names.push(fam_name.clone());
        Ok(fam_name)
    }

    fn contains(&self, family: &FontFamily) -> bool {
        self.names.contains(family)
    }

    fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    fn collection(&mut self) -> &FontCollection {
        if self.collection.is_none() {
            let loader = CustomFontCollectionLoaderImpl::new(self.files.as_slice());
            let collection = FontCollection::from_loader(loader);
            self.collection = Some(collection);
        }
        self.collection.as_ref().unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;

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
        let a_font = FontFamily::new_unchecked("Segoe UI");
        let layout = D2DText::new_for_test()
            .new_text_layout("")
            .font(a_font, 12.0)
            .build()
            .unwrap();
        let pt = layout.hit_test_point(Point::new(0.0, 0.0));
        assert_eq!(pt.idx, 0);
        let pos = layout.hit_test_text_position(0);
        assert_eq!(pos.point.x, 0.0);
        assert_close!(pos.point.y, 10.0, 3.0);
        let line = layout.line_metric(0).unwrap();
        assert_close!(line.height, 14.0, 3.0);
    }

    #[test]
    fn test_hit_test_text_position_basic() {
        let mut text_layout = D2DText::new_for_test();

        let input = "piet text!";
        let font = text_layout.font_family("Segoe UI").unwrap();

        let layout = text_layout
            .new_text_layout(&input[0..4])
            .font(font.clone(), 12.0)
            .build()
            .unwrap();
        let piet_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[0..3])
            .font(font.clone(), 12.0)
            .build()
            .unwrap();
        let pie_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[0..2])
            .font(font.clone(), 12.0)
            .build()
            .unwrap();
        let pi_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[0..1])
            .font(font.clone(), 12.0)
            .build()
            .unwrap();
        let p_width = layout.size().width;

        let layout = text_layout
            .new_text_layout("")
            .font(font.clone(), 12.0)
            .build()
            .unwrap();
        let null_width = layout.size().width;

        let full_layout = text_layout
            .new_text_layout(input)
            .font(font, 12.0)
            .build()
            .unwrap();
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
    }

    #[test]
    fn test_hit_test_text_position_complex_0() {
        let mut text_layout = D2DText::new_for_test();

        let input = "é";
        assert_eq!(input.len(), 2);

        let font = text_layout.font_family("Segoe UI").unwrap();
        let layout = text_layout
            .new_text_layout(input)
            .font(font, 12.0)
            .build()
            .unwrap();

        assert_close!(layout.hit_test_text_position(0).point.x, 0.0, 3.0);
        assert_close!(
            layout.hit_test_text_position(2).point.x,
            layout.size().width,
            3.0,
        );

        // unicode segmentation is wrong on this one for now.
        //let input = "🤦\u{1f3fc}\u{200d}\u{2642}\u{fe0f}";

        //let mut text_layout = D2DText::new();
        //let font = text_layout.new_font_by_name("sans-serif", 12.0).build();
        //let layout = text_layout.new_text_layout(&font, input, None).build();

        //assert_eq!(input.graphemes(true).count(), 1);
        //assert_eq!(layout.hit_test_text_position(0, true).map(|p| p.point_x), Some(layout.size().width));
        //assert_eq!(input.len(), 17);

        let input = "\u{0023}\u{FE0F}\u{20E3}"; // #️⃣
        assert_eq!(input.len(), 7);
        assert_eq!(input.chars().count(), 3);

        let mut text_layout = D2DText::new_for_test();

        let font = text_layout.font_family("Segoe UI").unwrap();
        let layout = text_layout
            .new_text_layout(input)
            .font(font, 12.0)
            .build()
            .unwrap();

        assert_close!(layout.hit_test_text_position(0).point.x, 0.0, 3.0);
        assert_close!(
            layout.hit_test_text_position(7).point.x,
            layout.size().width,
            3.0,
        );

        // note code unit not at grapheme boundary
        assert_close!(layout.hit_test_text_position(1).point.x, 0.0, 3.0);
    }

    #[test]
    fn test_hit_test_text_position_complex_1() {
        let mut text_layout = D2DText::new_for_test();

        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇
        assert_eq!(input.len(), 14);

        let font = text_layout.font_family("Segoe UI").unwrap();
        let layout = text_layout
            .new_text_layout(input)
            .font(font, 12.0)
            .build()
            .unwrap();

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
    fn test_hit_test_point_basic() {
        let mut text_layout = D2DText::new_for_test();

        let font = text_layout.font_family("Segoe UI").unwrap();
        let layout = text_layout
            .new_text_layout("piet text!")
            .font(font, 12.0)
            .build()
            .unwrap();
        println!("text pos 4: {:?}", layout.hit_test_text_position(4)); // 20.302734375
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 23.58984375
        println!("text pos 6: {:?}", layout.hit_test_text_position(6)); // 23.58984375

        // test hit test point
        // all inside
        let pt = layout.hit_test_point(Point::new(21.0, 0.0));
        assert_eq!(pt.idx, 4);
        let pt = layout.hit_test_point(Point::new(22.0, 0.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(24.0, 0.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(25.0, 0.0));
        assert_eq!(pt.idx, 5);

        // outside
        println!("layout_width: {:?}", layout.size().width); // 46.916015625

        let pt = layout.hit_test_point(Point::new(48.0, 0.0));
        assert_eq!(pt.idx, 10); // last text position
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(-1.0, 0.0));
        assert_eq!(pt.idx, 0); // first text position
        assert_eq!(pt.is_inside, false);
    }

    #[test]
    fn test_hit_test_point_complex() {
        let mut text_layout = D2DText::new_for_test();

        // Notes on this input:
        // 6 code points
        // 7 utf-16 code units (1/1/1/1/1/2)
        // 14 utf-8 code units (2/1/3/3/1/4)
        // 4 graphemes
        let input = "é\u{0023}\u{FE0F}\u{20E3}1\u{1D407}"; // #️⃣,, 𝐇
        let font = text_layout.font_family("Segoe UI").unwrap();
        let layout = text_layout
            .new_text_layout(input)
            .font(font, 12.0)
            .build()
            .unwrap();
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 6.275390625
        println!("text pos 9: {:?}", layout.hit_test_text_position(9)); // 18.0
        println!("text pos 10: {:?}", layout.hit_test_text_position(10)); // 24.46875
        println!("text pos 14: {:?}", layout.hit_test_text_position(14)); // 33.3046875, line width

        let pt = layout.hit_test_point(Point::new(2.0, 0.0));
        assert_eq!(pt.idx, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(7.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(10.0, 0.0));
        assert_eq!(pt.idx, 2);
        let pt = layout.hit_test_point(Point::new(14.0, 0.0));
        assert_eq!(pt.idx, 9);
        let pt = layout.hit_test_point(Point::new(18.0, 0.0));
        assert_eq!(pt.idx, 9);
        let pt = layout.hit_test_point(Point::new(19.0, 0.0));
        assert_eq!(pt.idx, 9);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(25.0, 0.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(32.0, 0.0));
        assert_eq!(pt.idx, 14);
        let pt = layout.hit_test_point(Point::new(35.0, 0.0));
        assert_eq!(pt.idx, 14);
    }

    #[test]
    fn test_basic_multiline() {
        let input = "piet text most best";
        let width_small = 30.0;

        let mut text_layout = D2DText::new_for_test();
        let font = text_layout.font_family("Segoe UI").unwrap();
        let layout = text_layout
            .new_text_layout(input)
            .max_width(width_small)
            .font(font, 12.0)
            .build()
            .unwrap();

        assert_eq!(layout.line_count(), 4);
        assert_eq!(layout.line_text(0), Some("piet"));
        assert_eq!(layout.line_text(1), Some("text"));
        assert_eq!(layout.line_text(2), Some("most"));
        assert_eq!(layout.line_text(3), Some("best"));
        assert_eq!(layout.line_text(4), None);
    }

    // NOTE be careful, windows will break lines at the sub-word level!
    #[test]
    fn test_multiline_hit_test_text_position_basic() {
        let mut text_layout = D2DText::new_for_test();

        let input = "piet  text!";
        let font = text_layout.font_family("Segoe UI").unwrap();

        let layout = text_layout
            .new_text_layout(&input[0..4])
            .font(font.clone(), 15.0)
            .max_width(30.0)
            .build()
            .unwrap();
        let piet_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[0..3])
            .font(font.clone(), 15.0)
            .max_width(30.0)
            .build()
            .unwrap();
        let pie_width = layout.size().width;

        let layout = text_layout.new_text_layout(&input[0..5]).build().unwrap();
        let piet_space_width = layout.size().width;

        // "text" should be on second line
        let layout = text_layout
            .new_text_layout(&input[6..10])
            .font(font.clone(), 15.0)
            .max_width(30.0)
            .build()
            .unwrap();
        let text_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[6..9])
            .font(font.clone(), 15.0)
            .max_width(30.0)
            .build()
            .unwrap();
        let tex_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[6..8])
            .max_width(30.0)
            .build()
            .unwrap();
        let te_width = layout.size().width;

        let layout = text_layout
            .new_text_layout(&input[6..7])
            .font(font.clone(), 15.0)
            .max_width(30.0)
            .build()
            .unwrap();
        let t_width = layout.size().width;

        let full_layout = text_layout
            .new_text_layout(input)
            .font(font, 15.0)
            .max_width(30.0)
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
        let line_zero_metric = full_layout.line_metric(0).unwrap();
        let line_one_metric = full_layout.line_metric(1).unwrap();
        let line_zero_baseline = line_zero_metric.y_offset + line_zero_metric.baseline;
        let line_one_baseline = line_one_metric.y_offset + line_one_metric.baseline;

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

        // This tests that hit-testing trailing whitespace can return points
        // outside of the layout's reported width.
        assert!(full_layout.hit_test_text_position(5).point.x > piet_space_width + 3.0,);

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
    // very basic testing that multiline works
    fn test_multiline_hit_test_point_basic() {
        let input = "piet text most best";

        let mut text = D2DText::new_for_test();

        let font = text.font_family("Segoe UI").unwrap();
        // this should break into four lines
        let layout = text
            .new_text_layout(input)
            .font(font, 12.0)
            .max_width(30.0)
            .build()
            .unwrap();
        println!("{}", layout.line_metric(0).unwrap().baseline); // 12.94...
        println!("text pos 01: {:?}", layout.hit_test_text_position(0)); // (0.0, 12.94)
        println!("text pos 06: {:?}", layout.hit_test_text_position(5)); // (0.0, 28.91...)
        println!("text pos 11: {:?}", layout.hit_test_text_position(10)); // (0.0, 44.87...)
        println!("text pos 16: {:?}", layout.hit_test_text_position(15)); // (0.0, 60.83...)

        let pt = layout.hit_test_point(Point::new(1.0, -13.0)); // under
        assert_eq!(pt.idx, 0);
        assert_eq!(pt.is_inside, false);
        let pt = layout.hit_test_point(Point::new(1.0, 1.0));
        assert_eq!(pt.idx, 0);
        assert_eq!(pt.is_inside, true);
        let pt = layout.hit_test_point(Point::new(1.0, 00.0));
        assert_eq!(pt.idx, 0);
        let pt = layout.hit_test_point(Point::new(1.0, 20.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(1.0, 36.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(1.0, 54.0));
        assert_eq!(pt.idx, 15);

        // over on y axis, but x still affects the text position
        let best_layout = text.new_text_layout("best").build().unwrap();
        println!("layout width: {:#?}", best_layout.size().width); // 22.48...

        let pt = layout.hit_test_point(Point::new(1.0, 68.0));
        assert_eq!(pt.idx, 15);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(22.0, 68.0));
        assert_eq!(pt.idx, 19);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(24.0, 68.0));
        assert_eq!(pt.idx, 19);
        assert_eq!(pt.is_inside, false);

        // under
        let piet_layout = text.new_text_layout("piet ").build().unwrap();
        println!("layout width: {:#?}", piet_layout.size().width); // 23.58...

        let pt = layout.hit_test_point(Point::new(1.0, -14.0)); // under
        assert_eq!(pt.idx, 0);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(23.0, -14.0)); // under
        assert_eq!(pt.idx, 5);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(27.0, -14.0)); // under
        assert_eq!(pt.idx, 5);
        assert_eq!(pt.is_inside, false);
    }

    #[test]
    fn missing_font_is_missing() {
        let mut text = D2DText::new_for_test();
        assert!(text.font_family("A Quite Unlikely Font Ñame").is_none());
    }
}
