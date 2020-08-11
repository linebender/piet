//! Text functionality for Piet direct2d backend

mod lines;

use std::cell::RefCell;
use std::convert::TryInto;
use std::ops::{Range, RangeBounds};
use std::rc::Rc;
use std::sync::Arc;

pub use d2d::{D2DDevice, D2DFactory, DeviceContext as D2DDeviceContext};
pub use dwrite::DwriteFactory;
use dwrote::{CustomFontCollectionLoaderImpl, FontCollection, FontFile};
use wio::wide::ToWide;

use piet::kurbo::{Insets, Point, Rect, Size};
use piet::util;
use piet::{
    Error, FontFamily, HitTestPoint, HitTestPosition, LineMetric, Text, TextAlignment,
    TextAttribute, TextLayout, TextLayoutBuilder,
};

use crate::conv;
use crate::d2d;
use crate::dwrite::{self, TextFormat};

#[derive(Clone)]
pub struct D2DText {
    dwrite: DwriteFactory,
    device: d2d::DeviceContext,
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
    pub text: String,
    // currently calculated on build
    line_metrics: Vec<LineMetric>,
    size: Size,
    /// insets that, when applied to our layout rect, generates our inking/image rect.
    inking_insets: Insets,
    pub layout: dwrite::TextLayout,
}

pub struct D2DTextLayoutBuilder {
    text: String,
    layout: Result<dwrite::TextLayout, Error>,
    len_utf16: usize,
    device: d2d::DeviceContext,
    loaded_fonts: Rc<RefCell<LoadedFonts>>,
}

impl D2DText {
    /// Create a new factory that satisfies the piet `Text` trait given
    /// the (platform-specific) dwrite factory.
    pub fn new(dwrite: DwriteFactory, device: d2d::DeviceContext) -> D2DText {
        D2DText {
            dwrite,
            device,
            loaded_fonts: Default::default(),
        }
    }

    #[cfg(test)]
    pub fn new_for_test() -> D2DText {
        let d2d = d2d::D2DFactory::new().unwrap();
        let dwrite = DwriteFactory::new().unwrap();
        // Initialize a D3D Device
        let (d3d, _d3d_ctx) = crate::d3d::D3D11Device::create().unwrap();

        // Create the D2D Device and Context
        let mut device = unsafe { d2d.create_device(d3d.as_dxgi().unwrap().as_raw()).unwrap() };
        let device = device.create_device_context().unwrap();
        D2DText::new(dwrite, device)
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

    fn new_text_layout(&mut self, text: &str) -> Self::TextLayoutBuilder {
        let width = f32::INFINITY;
        let wide_str = text.to_wide();
        let layout = TextFormat::new(&self.dwrite, &[], util::DEFAULT_FONT_SIZE as f32)
            .and_then(|format| dwrite::TextLayout::new(&self.dwrite, format, width, &wide_str))
            .map_err(Into::into);

        D2DTextLayoutBuilder {
            layout,
            text: text.to_owned(),
            len_utf16: wide_str.len(),
            device: self.device.clone(),
            loaded_fonts: self.loaded_fonts.clone(),
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
        self.add_attribute_shared(attribute.into(), None);
        self
    }

    fn range_attribute(
        mut self,
        range: impl RangeBounds<usize>,
        attribute: impl Into<TextAttribute>,
    ) -> Self {
        let range = util::resolve_range(range, self.text.len());
        let attribute = attribute.into();

        self.add_attribute_shared(attribute, Some(range));
        self
    }

    fn build(self) -> Result<Self::Out, Error> {
        let layout = self.layout?;
        let line_metrics = lines::fetch_line_metrics(&self.text, &layout);
        let text_metrics = layout.get_metrics();
        let overhang = layout.get_overhang_metrics();

        let size = Size::new(text_metrics.width as f64, text_metrics.height as f64);
        let overhang_width = text_metrics.layoutWidth as f64 + overhang.x1;
        let overhang_height = text_metrics.layoutHeight as f64 + overhang.y1;

        let inking_insets = Insets::new(
            overhang.x0,
            overhang.y0,
            overhang_width - size.width,
            overhang_height - size.height,
        );

        Ok(D2DTextLayout {
            text: self.text,
            line_metrics,
            layout,
            size,
            inking_insets,
        })
    }
}

impl D2DTextLayoutBuilder {
    /// used for both range and default attributes
    fn add_attribute_shared(&mut self, attr: TextAttribute, range: Option<Range<usize>>) {
        if let Ok(layout) = self.layout.as_mut() {
            let (start, len) = match range {
                Some(range) => {
                    let start = util::count_utf16(&self.text[..range.start]);
                    let len = if range.end == self.text.len() {
                        self.len_utf16
                    } else {
                        util::count_utf16(&self.text[range])
                    };
                    (start, len)
                }
                None => (0, self.len_utf16),
            };

            match attr {
                TextAttribute::Font(font) => {
                    let is_custom = self.loaded_fonts.borrow().contains(&font);
                    if is_custom {
                        let mut loaded = self.loaded_fonts.borrow_mut();
                        layout.set_font_collection(start, len, loaded.collection());
                    } else if !self.loaded_fonts.borrow().is_empty() {
                        // if we are using custom fonts we also need to set the collection
                        // back to the system collection explicity as needed
                        layout.set_font_collection(start, len, &FontCollection::system());
                    }
                    let family_name = resolve_family_name(&font);
                    layout.set_font_family(start, len, family_name);
                }
                TextAttribute::Size(size) => layout.set_size(start, len, size as f32),
                TextAttribute::Weight(weight) => layout.set_weight(start, len, weight),
                TextAttribute::Italic(flag) => layout.set_italic(start, len, flag),
                TextAttribute::Underline(flag) => layout.set_underline(start, len, flag),
                TextAttribute::ForegroundColor(color) => {
                    if let Ok(brush) = self.device.create_solid_color(conv::color_to_colorf(color))
                    {
                        layout.set_foregound_brush(start, len, brush)
                    }
                }
            }
        }
    }
}

impl TextLayout for D2DTextLayout {
    fn width(&self) -> f64 {
        self.size.width
    }

    fn size(&self) -> Size {
        self.size
    }

    fn image_bounds(&self) -> Rect {
        self.size.to_rect() + self.inking_insets
    }

    fn text(&self) -> &str {
        &self.text
    }

    /// given a new max width, update width of text layout to fit within the max width
    // TODO add this doc to trait method? or is this windows specific?
    fn update_width(&mut self, new_width: impl Into<Option<f64>>) -> Result<(), Error> {
        let new_width = new_width.into().unwrap_or(std::f64::INFINITY);

        self.layout.set_max_width(new_width)?;
        self.line_metrics = lines::fetch_line_metrics(&self.text, &self.layout);

        Ok(())
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        self.line_metrics
            .get(line_number)
            .map(|lm| &self.text[lm.start_offset..(lm.end_offset - lm.trailing_whitespace)])
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        self.line_metrics.get(line_number).cloned()
    }

    fn line_count(&self) -> usize {
        self.line_metrics.len()
    }

    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        // Before hit testing, need to convert point.y to have 0.0 at upper left corner (dwrite
        // style) instead of at first line baseline.
        let first_baseline = self
            .line_metrics
            .get(0)
            .map(|lm| lm.baseline)
            .unwrap_or(0.0);
        let y = point.y + first_baseline;

        // lossy from f64 to f32, but shouldn't have too much impact
        let htp = self.layout.hit_test_point(point.x as f32, y as f32);

        // Round up to next grapheme cluster boundary if directwrite
        // reports a trailing hit.
        let text_position_16 = if htp.is_trailing_hit {
            htp.metrics.text_position + htp.metrics.length
        } else {
            htp.metrics.text_position
        } as usize;

        // Convert text position from utf-16 code units to
        // utf-8 code units.
        // Strategy: count up in utf16 and utf8 simultaneously, stop when
        // utf-16 text position reached.
        //
        // TODO ask about text_position, it looks like windows returns last index;
        // can't use the text_position of last index from directwrite, it has an extra code unit.
        let text_position = util::count_until_utf16(&self.text, text_position_16)
            .unwrap_or_else(|| self.text.len());

        HitTestPoint::new(text_position, htp.is_inside)
    }

    // Can panic if text position is not at a code point boundary, or if it's out of bounds.
    fn hit_test_text_position(&self, text_position: usize) -> Option<HitTestPosition> {
        // Note: Directwrite will just return the line width if text position is
        // out of bounds. This is what want for piet; return line width for the last text position
        // (equal to line.len()). This is basically returning line width for the last cursor
        // position.

        // Now convert the utf8 index to utf16.
        // This can panic;
        let idx_16 = util::count_utf16(&self.text[0..text_position]);
        let line = util::line_number_for_position(&self.line_metrics, text_position);
        // panic or Result are also fine options for dealing with overflow. Using Option here
        // because it's already present and convenient.
        // TODO this should probably go before convertin to utf16, since that's relatively slow
        let idx_16 = idx_16.try_into().ok()?;

        let trailing = false;

        self.layout
            .hit_test_text_position(idx_16, trailing)
            .map(|hit| {
                HitTestPosition::new(Point::new(hit.point_x as f64, hit.point_y as f64), line)
            })
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
            full_layout.hit_test_text_position(4).unwrap().point.x,
            piet_width,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(3).unwrap().point.x,
            pie_width,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(2).unwrap().point.x,
            pi_width,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(1).unwrap().point.x,
            p_width,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(0).unwrap().point.x,
            null_width,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(10).unwrap().point.x,
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

        assert_close!(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close!(
            layout.hit_test_text_position(2).unwrap().point.x,
            layout.size().width,
            3.0,
        );

        // unicode segmentation is wrong on this one for now.
        //let input = "🤦\u{1f3fc}\u{200d}\u{2642}\u{fe0f}";

        //let mut text_layout = D2DText::new();
        //let font = text_layout.new_font_by_name("sans-serif", 12.0).build().unwrap();
        //let layout = text_layout.new_text_layout(&font, input, None).build().unwrap();

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

        assert_close!(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close!(
            layout.hit_test_text_position(7).unwrap().point.x,
            layout.size().width,
            3.0,
        );

        // note code unit not at grapheme boundary
        assert_close!(layout.hit_test_text_position(1).unwrap().point.x, 0.0, 3.0);
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
        assert_close!(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close!(
            layout.hit_test_text_position(2).unwrap().point.x,
            test_layout_0.size().width,
            3.0,
        );
        assert_close!(
            layout.hit_test_text_position(9).unwrap().point.x,
            test_layout_1.size().width,
            3.0,
        );
        assert_close!(
            layout.hit_test_text_position(10).unwrap().point.x,
            test_layout_2.size().width,
            3.0,
        );
        assert_close!(
            layout.hit_test_text_position(14).unwrap().point.x,
            layout.size().width,
            3.0,
        );

        // Code point boundaries, but not grapheme boundaries.
        // Width should stay at the current grapheme boundary.
        assert_close!(
            layout.hit_test_text_position(3).unwrap().point.x,
            test_layout_0.size().width,
            3.0,
        );
        assert_close!(
            layout.hit_test_text_position(6).unwrap().point.x,
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

    #[test]
    fn test_change_width() {
        let input = "piet text most best";
        let width_small = 30.0;
        let width_medium = 60.0;
        let width_large = 1000.0;

        let mut text_layout = D2DText::new_for_test();
        let font = text_layout.font_family("Segoe UI").unwrap();
        let mut layout = text_layout
            .new_text_layout(input)
            .font(font, 12.0)
            .max_width(width_small)
            .build()
            .unwrap();

        assert_eq!(layout.line_count(), 4);
        assert_eq!(layout.line_text(0), Some("piet"));

        layout.update_width(width_medium).unwrap();
        assert_eq!(layout.line_count(), 2);
        assert_eq!(layout.line_text(0), Some("piet text"));

        layout.update_width(width_large).unwrap();
        assert_eq!(layout.line_count(), 1);
        assert_eq!(layout.line_text(0), Some("piet text most best"));
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
        let line_zero_baseline = 0.0;
        let line_one_baseline = full_layout.line_metric(1).unwrap().height;

        // these just test the x position of text positions on the second line
        assert_close!(
            full_layout.hit_test_text_position(10).unwrap().point.x,
            text_width,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(9).unwrap().point.x,
            tex_width,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(8).unwrap().point.x,
            te_width,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(7).unwrap().point.x,
            t_width,
            3.0,
        );
        // This should be beginning of second line
        assert_close!(
            full_layout.hit_test_text_position(6).unwrap().point.x,
            0.0,
            3.0,
        );

        assert_close!(
            full_layout.hit_test_text_position(3).unwrap().point.x,
            pie_width,
            3.0,
        );

        // This tests that hit-testing trailing whitespace can return points
        // outside of the layout's reported width.
        assert!(full_layout.hit_test_text_position(5).unwrap().point.x > piet_space_width + 3.0,);

        // These test y position of text positions on line 1 (0-index)
        assert_close!(
            full_layout.hit_test_text_position(10).unwrap().point.y,
            line_one_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(9).unwrap().point.y,
            line_one_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(8).unwrap().point.y,
            line_one_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(7).unwrap().point.y,
            line_one_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(6).unwrap().point.y,
            line_one_baseline,
            3.0,
        );

        // this tests y position of 0 line
        assert_close!(
            full_layout.hit_test_text_position(5).unwrap().point.y,
            line_zero_baseline,
            3.0,
        );
        assert_close!(
            full_layout.hit_test_text_position(4).unwrap().point.y,
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
        println!("text pos 01: {:?}", layout.hit_test_text_position(0)); // (0.0, 0.0)
        println!("text pos 06: {:?}", layout.hit_test_text_position(5)); // (0.0, 15.96...)
        println!("text pos 11: {:?}", layout.hit_test_text_position(10)); // (0.0, 31.92...)
        println!("text pos 16: {:?}", layout.hit_test_text_position(15)); // (0.0, 47.88...)

        let pt = layout.hit_test_point(Point::new(1.0, -13.0)); // under
        assert_eq!(pt.idx, 0);
        assert_eq!(pt.is_inside, false);
        let pt = layout.hit_test_point(Point::new(1.0, -1.0));
        println!("{:?}", pt);
        assert_eq!(pt.idx, 0);
        assert_eq!(pt.is_inside, true);
        let pt = layout.hit_test_point(Point::new(1.0, 00.0));
        assert_eq!(pt.idx, 0);
        let pt = layout.hit_test_point(Point::new(1.0, 04.0));
        assert_eq!(pt.idx, 5);
        let pt = layout.hit_test_point(Point::new(1.0, 20.0));
        assert_eq!(pt.idx, 10);
        let pt = layout.hit_test_point(Point::new(1.0, 36.0));
        assert_eq!(pt.idx, 15);

        // over on y axis, but x still affects the text position
        let best_layout = text.new_text_layout("best").build().unwrap();
        println!("layout width: {:#?}", best_layout.size().width); // 22.48...

        let pt = layout.hit_test_point(Point::new(1.0, 52.0));
        assert_eq!(pt.idx, 15);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(22.0, 52.0));
        assert_eq!(pt.idx, 19);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(24.0, 52.0));
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
