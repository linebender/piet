//! Text related stuff for the coregraphics backend

use std::collections::HashMap;
use std::fmt;
use std::ops::{Range, RangeBounds};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use core_foundation::base::TCFType;
use core_foundation::dictionary::{CFDictionary, CFMutableDictionary};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation_sys::base::CFRange;
use core_graphics::base::CGFloat;
use core_graphics::context::CGContextRef;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::path::CGPath;
use core_text::{
    font,
    font::CTFont,
    font_descriptor::{self, SymbolicTraitAccessors},
    string_attributes,
};

use piet::kurbo::{Affine, Point, Rect, Size};
use piet::{
    util, Error, FontFamily, FontStyle, FontWeight, HitTestPoint, HitTestPosition, LineMetric,
    Text, TextAlignment, TextAttribute, TextLayout, TextLayoutBuilder, TextStorage,
};

use crate::ct_helpers::{self, AttributedString, FontCollection, Frame, Framesetter, Line};

/// both infinity and f64::MAX produce unpleasant results
const MAX_LAYOUT_CONSTRAINT: f64 = 1e9;

#[derive(Clone)]
pub struct CoreGraphicsText {
    shared: SharedTextState,
}

/// State shared by all `CoreGraphicsText` objects.
///
/// This is for holding onto expensive to create objects, and for things
/// like caching fonts.
#[derive(Clone)]
struct SharedTextState {
    inner: Arc<Mutex<TextState>>,
}

struct TextState {
    collection: FontCollection,
    family_cache: HashMap<String, Option<FontFamily>>,
}

#[derive(Clone)]
pub struct CoreGraphicsTextLayout {
    text: Rc<dyn TextStorage>,
    attr_string: AttributedString,
    framesetter: Framesetter,
    pub(crate) frame: Option<Frame>,
    /// The size of our layout as understood by coretext
    pub(crate) frame_size: Size,
    /// Extra height that is not part of our coretext frame. This can be from
    /// one of two things: either the height of an empty layout, or the height
    /// of the implied extra line when the layout ends in a newline.
    bonus_height: f64,
    image_bounds: Rect,
    width_constraint: f64,
    // these two are stored values we use to determine cursor extents when the layout is empty.
    default_baseline: f64,
    default_line_height: f64,
    line_metrics: Rc<[LineMetric]>,
    x_offsets: Rc<[f64]>,
    trailing_ws_width: f64,
}

/// Building text layouts for `CoreGraphics`.
pub struct CoreGraphicsTextLayoutBuilder {
    width: f64,
    alignment: TextAlignment,
    text: Rc<dyn TextStorage>,
    /// the end bound up to which we have already added attrs to our AttributedString
    last_resolved_pos: usize,
    last_resolved_utf16: usize,
    attr_string: AttributedString,
    /// We set default attributes once on the underlying attributed string;
    /// this happens either when the first range attribute is added, or when
    /// we build the string.
    has_set_default_attrs: bool,
    default_baseline: f64,
    default_line_height: f64,
    attrs: Attributes,
}

/// A helper type for storing and resolving attributes
#[derive(Default)]
struct Attributes {
    defaults: util::LayoutDefaults,
    font: Option<Span<FontFamily>>,
    size: Option<Span<f64>>,
    weight: Option<Span<FontWeight>>,
    style: Option<Span<FontStyle>>,
}

/// during construction, `Span`s represent font attributes that have been applied
/// to ranges of the text; these are combined into coretext font objects as the
/// layout is built.
struct Span<T> {
    payload: T,
    range: Range<usize>,
}

impl<T> Span<T> {
    fn new(payload: T, range: Range<usize>) -> Self {
        Span { payload, range }
    }

    fn range_end(&self) -> usize {
        self.range.end
    }
}

impl CoreGraphicsTextLayoutBuilder {
    /// ## Note
    ///
    /// The implementation of this has a few particularities.
    ///
    /// The main Foundation type for representing a rich text string is NSAttributedString
    /// (CFAttributedString in CoreFoundation); however not all attributes are set
    /// directly. Attributes that implicate font selection (such as size, weight, etc)
    /// are all part of the string's 'font' attribute; we can't set them individually.
    ///
    /// To make this work, we keep track of the active value for each of the relevant
    /// attributes. Each span of the string with a common set of these values is assigned
    /// the appropriate concrete font as the attributes are added.
    ///
    /// This behaviour relies on the condition that spans are added in non-decreasing
    /// start order. The algorithm is quite simple; whenever a new attribute of one
    /// of the relevant types is added, we know that spans in the string up to
    /// the start of the newly added span can no longer be changed, and we can resolve them.
    fn add(&mut self, attr: TextAttribute, range: Range<usize>) {
        if !self.has_set_default_attrs {
            self.set_default_attrs();
        }
        // Some attributes are 'standalone' and can just be added to the attributed string
        // immediately.
        if matches!(
            &attr,
            TextAttribute::TextColor(_) | TextAttribute::Underline(_)
        ) {
            return self.add_immediately(attr, range);
        }

        debug_assert!(
            range.start >= self.last_resolved_pos,
            "attributes must be added with non-decreasing start positions"
        );

        self.resolve_up_to(range.start);
        // Other attributes need to be handled incrementally, since they all participate
        // in creating the CTFont objects
        self.attrs.add(range, attr);
    }

    fn set_default_attrs(&mut self) {
        self.has_set_default_attrs = true;
        let whole_range = self.attr_string.range();
        let font = self.current_font();
        let height = compute_line_height(font.ascent(), font.descent(), font.leading());
        self.default_line_height = height;
        self.default_baseline = (font.ascent() + 0.5).floor();
        self.attr_string.set_font(whole_range, &font);
        self.attr_string
            .set_fg_color(whole_range, &self.attrs.defaults.fg_color);
        self.attr_string
            .set_underline(whole_range, self.attrs.defaults.underline);
    }

    fn add_immediately(&mut self, attr: TextAttribute, range: Range<usize>) {
        let utf16_start = util::count_utf16(&self.text[..range.start]);
        let utf16_len = util::count_utf16(&self.text[range]);
        let range = CFRange::init(utf16_start as isize, utf16_len as isize);
        match attr {
            TextAttribute::TextColor(color) => {
                self.attr_string.set_fg_color(range, &color);
            }
            TextAttribute::Underline(flag) => self.attr_string.set_underline(range, flag),
            _ => unreachable!(),
        }
    }

    fn finalize(&mut self) {
        if !self.has_set_default_attrs {
            self.set_default_attrs();
        }
        self.resolve_up_to(self.text.len());
    }

    /// Add all font attributes up to a boundary.
    fn resolve_up_to(&mut self, resolve_end: usize) {
        let mut next_span_end = self.last_resolved_pos;
        while next_span_end < resolve_end {
            next_span_end = self.next_span_end(resolve_end);
            if next_span_end > self.last_resolved_pos {
                let range_end_utf16 =
                    util::count_utf16(&self.text[self.last_resolved_pos..next_span_end]);
                let range =
                    CFRange::init(self.last_resolved_utf16 as isize, range_end_utf16 as isize);
                let font = self.current_font();
                unsafe {
                    self.attr_string.inner.set_attribute(
                        range,
                        string_attributes::kCTFontAttributeName,
                        &font,
                    );
                }
                self.last_resolved_pos = next_span_end;
                self.last_resolved_utf16 += range_end_utf16;
                self.update_after_adding_span();
            }
        }
    }

    /// Given the end of a range, return the min of that value and the ends of
    /// any existing spans.
    ///
    /// ## Invariant
    ///
    /// It is an invariant that the end range of any `FontAttr` is greater than
    /// `self.last_resolved_pos`
    fn next_span_end(&self, max: usize) -> usize {
        self.attrs.next_span_end(max)
    }

    /// returns the fully constructed font object, including weight and size.
    ///
    /// This is stateful; it depends on the current attributes being correct
    /// for the range that begins at `self.last_resolved_pos`.
    fn current_font(&self) -> CTFont {
        //TODO: this is where caching would happen, if we were implementing caching;
        //store a tuple of attributes resolved to a generated CTFont.

        // 'wght' as an int
        const WEIGHT_AXIS_TAG: i32 = make_opentype_tag("wght") as i32;
        // taken from android:
        // https://api.skia.org/classSkFont.html#aa85258b584e9c693d54a8624e0fe1a15
        const SLANT_TANGENT: f64 = 0.25;

        unsafe {
            let family_key =
                CFString::wrap_under_create_rule(font_descriptor::kCTFontFamilyNameAttribute);
            let family_name = ct_helpers::ct_family_name(self.attrs.font(), self.attrs.size());
            let weight_key = CFString::wrap_under_create_rule(font_descriptor::kCTFontWeightTrait);
            let weight = convert_to_coretext(self.attrs.weight());

            let traits_key =
                CFString::wrap_under_create_rule(font_descriptor::kCTFontTraitsAttribute);
            let mut traits = CFMutableDictionary::new();
            traits.set(weight_key, weight.as_CFType());
            if self.attrs.italic() {
                let symbolic_traits_key =
                    CFString::wrap_under_create_rule(font_descriptor::kCTFontSymbolicTrait);
                let symbolic_traits = CFNumber::from(font_descriptor::kCTFontItalicTrait as i32);
                traits.set(symbolic_traits_key, symbolic_traits.as_CFType());
            }

            let attributes = CFDictionary::from_CFType_pairs(&[
                (family_key, family_name.as_CFType()),
                (traits_key, traits.as_CFType()),
            ]);
            let descriptor = font_descriptor::new_from_attributes(&attributes);
            let font = font::new_from_descriptor(&descriptor, self.attrs.size());

            let needs_synthetic_ital = self.attrs.italic() && !font.symbolic_traits().is_italic();
            let has_var_axes = font.get_variation_axes().is_some();

            if !(needs_synthetic_ital | has_var_axes) {
                return font;
            }

            let affine = if needs_synthetic_ital {
                Affine::new([1.0, 0.0, SLANT_TANGENT, 1.0, 0., 0.])
            } else {
                Affine::default()
            };

            let variation_axes = font
                .get_variation_axes()
                .map(|axes| {
                    axes.iter()
                        .flat_map(|dict| {
                            // for debugging, this is how you get the name for the axis
                            //let name = dict.find(ct_helpers::kCTFontVariationAxisNameKey).and_then(|v| v.downcast::<CFString>());
                            dict.find(ct_helpers::kCTFontVariationAxisIdentifierKey)
                                .and_then(|v| v.downcast::<CFNumber>().and_then(|num| num.to_i32()))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            // only set weight axis if it exists, and we're not a system font (things get weird)
            let descriptor =
                if variation_axes.contains(&WEIGHT_AXIS_TAG) && !self.attrs.font().is_generic() {
                    let weight_axis_id: CFNumber = WEIGHT_AXIS_TAG.into();
                    let descriptor = font_descriptor::CTFontDescriptorCreateCopyWithVariation(
                        descriptor.as_concrete_TypeRef(),
                        weight_axis_id.as_concrete_TypeRef(),
                        self.attrs.weight().to_raw() as _,
                    );
                    font_descriptor::CTFontDescriptor::wrap_under_create_rule(descriptor)
                } else {
                    descriptor
                };

            ct_helpers::make_font(&descriptor, self.attrs.size(), affine)
        }
    }

    /// After we have added a span, check to see if any of our attributes are no
    /// longer active.
    ///
    /// This is stateful; it requires that `self.last_resolved_pos` has been just updated
    /// to reflect the end of the span just added.
    fn update_after_adding_span(&mut self) {
        self.attrs.clear_up_to(self.last_resolved_pos)
    }
}

impl Attributes {
    fn add(&mut self, range: Range<usize>, attr: TextAttribute) {
        match attr {
            TextAttribute::FontFamily(font) => self.font = Some(Span::new(font, range)),
            TextAttribute::Weight(w) => self.weight = Some(Span::new(w, range)),
            TextAttribute::FontSize(s) => self.size = Some(Span::new(s, range)),
            TextAttribute::Style(s) => self.style = Some(Span::new(s, range)),
            TextAttribute::Strikethrough(_) => { /* Unimplemented for now as coregraphics doesn't have native strikethrough support. */
            }
            _ => unreachable!(),
        }
    }

    fn size(&self) -> f64 {
        self.size
            .as_ref()
            .map(|s| s.payload)
            .unwrap_or(self.defaults.font_size)
    }

    fn weight(&self) -> FontWeight {
        self.weight
            .as_ref()
            .map(|w| w.payload)
            .unwrap_or(self.defaults.weight)
    }

    fn italic(&self) -> bool {
        matches!(
            self.style
                .as_ref()
                .map(|t| t.payload)
                .unwrap_or(self.defaults.style),
            FontStyle::Italic
        )
    }

    fn font(&self) -> &FontFamily {
        self.font
            .as_ref()
            .map(|t| &t.payload)
            .unwrap_or_else(|| &self.defaults.font)
    }

    fn next_span_end(&self, max: usize) -> usize {
        self.font
            .as_ref()
            .map(Span::range_end)
            .unwrap_or(max)
            .min(self.size.as_ref().map(Span::range_end).unwrap_or(max))
            .min(self.weight.as_ref().map(Span::range_end).unwrap_or(max))
            .min(self.style.as_ref().map(Span::range_end).unwrap_or(max))
            .min(max)
    }

    // invariant: `last_pos` is the end of at least one span.
    fn clear_up_to(&mut self, last_pos: usize) {
        if self.font.as_ref().map(Span::range_end) == Some(last_pos) {
            self.font = None;
        }
        if self.weight.as_ref().map(Span::range_end) == Some(last_pos) {
            self.weight = None;
        }
        if self.style.as_ref().map(Span::range_end) == Some(last_pos) {
            self.style = None;
        }
        if self.size.as_ref().map(Span::range_end) == Some(last_pos) {
            self.size = None;
        }
    }
}

/// coretext uses a float in the range -1.0..=1.0, which has a non-linear mapping
/// to css-style weights. This is a fudge, adapted from QT:
///
/// https://git.sailfishos.org/mer-core/qtbase/commit/9ba296cc4cefaeb9d6c5abc2e0c0b272f2288733#1b84d1913347bd20dd0a134247f8cd012a646261_44_55
//TODO: a better solution would be piecewise linear interpolation between these values
fn convert_to_coretext(weight: FontWeight) -> CFNumber {
    match weight.to_raw() {
        0..=199 => -0.8,
        200..=299 => -0.6,
        300..=399 => -0.4,
        400..=499 => 0.0,
        500..=599 => 0.23,
        600..=699 => 0.3,
        700..=799 => 0.4,
        800..=899 => 0.56,
        _ => 0.62,
    }
    .into()
}

impl CoreGraphicsText {
    /// Create a new factory that satisfies the piet `Text` trait.
    ///
    /// The returned type will have freshly initiated inner state; this means
    /// it will not share a cache with any other objects created with this method.
    ///
    /// In general this should be created once and then cloned and passed around.
    pub fn new_with_unique_state() -> CoreGraphicsText {
        let collection = FontCollection::new_with_all_fonts();
        let inner = Arc::new(Mutex::new(TextState {
            collection,
            family_cache: HashMap::new(),
        }));
        CoreGraphicsText {
            shared: SharedTextState { inner },
        }
    }
}

impl fmt::Debug for CoreGraphicsText {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CoreGraphicsText").finish()
    }
}

impl Text for CoreGraphicsText {
    type TextLayout = CoreGraphicsTextLayout;
    type TextLayoutBuilder = CoreGraphicsTextLayoutBuilder;

    fn font_family(&mut self, family_name: &str) -> Option<FontFamily> {
        self.shared.get_font(family_name)
    }

    fn new_text_layout(&mut self, text: impl TextStorage) -> Self::TextLayoutBuilder {
        CoreGraphicsTextLayoutBuilder::new(text)
    }

    fn load_font(&mut self, data: &[u8]) -> Result<FontFamily, Error> {
        ct_helpers::add_font(data)
            .map(FontFamily::new_unchecked)
            .map_err(|_| Error::MissingFont)
    }
}

impl SharedTextState {
    /// return the family object for this family name, if it exists.
    ///
    /// This hits a cache before doing a lookup with the system.
    fn get_font(&mut self, family_name: &str) -> Option<FontFamily> {
        let mut inner = self.inner.lock().unwrap();
        match inner.family_cache.get(family_name) {
            Some(family) => family.clone(),
            None => {
                let maybe_family = inner.collection.font_for_family_name(family_name);
                inner
                    .family_cache
                    .insert(family_name.to_owned(), maybe_family.clone());
                maybe_family
            }
        }
    }
}

impl CoreGraphicsTextLayoutBuilder {
    fn new(text: impl TextStorage) -> Self {
        let text = Rc::new(text);
        let attr_string = AttributedString::new(text.as_str());
        CoreGraphicsTextLayoutBuilder {
            width: MAX_LAYOUT_CONSTRAINT,
            alignment: TextAlignment::default(),
            attrs: Default::default(),
            text,
            last_resolved_pos: 0,
            last_resolved_utf16: 0,
            attr_string,
            has_set_default_attrs: false,
            default_baseline: 0.0,
            default_line_height: 0.0,
        }
    }
}

impl fmt::Debug for CoreGraphicsTextLayoutBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CoreGraphicsTextLayoutBuilder").finish()
    }
}

impl TextLayoutBuilder for CoreGraphicsTextLayoutBuilder {
    type Out = CoreGraphicsTextLayout;

    fn max_width(mut self, width: f64) -> Self {
        self.width = width;
        self
    }

    fn tab_width(self, _width: f64) -> Self {
        self
    }

    fn alignment(mut self, alignment: piet::TextAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    fn default_attribute(mut self, attribute: impl Into<TextAttribute>) -> Self {
        debug_assert!(
            !self.has_set_default_attrs,
            "default attributes mut be added before range attributes"
        );
        let attribute = attribute.into();
        self.attrs.defaults.set(attribute);
        self
    }

    fn range_attribute(
        mut self,
        range: impl RangeBounds<usize>,
        attribute: impl Into<TextAttribute>,
    ) -> Self {
        let range = util::resolve_range(range, self.text.len());
        let attribute = attribute.into();
        self.add(attribute, range);
        self
    }

    fn build(mut self) -> Result<Self::Out, Error> {
        self.finalize();
        self.attr_string.set_alignment(self.alignment);
        Ok(CoreGraphicsTextLayout::new(
            self.text,
            self.attr_string,
            self.width,
            self.default_baseline,
            self.default_line_height,
        ))
    }
}

impl fmt::Debug for CoreGraphicsTextLayout {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CoreGraphicsTextLayout").finish()
    }
}

impl TextLayout for CoreGraphicsTextLayout {
    fn size(&self) -> Size {
        Size::new(
            self.frame_size.width,
            self.frame_size.height + self.bonus_height,
        )
    }

    fn trailing_whitespace_width(&self) -> f64 {
        self.trailing_ws_width
    }

    fn image_bounds(&self) -> Rect {
        self.image_bounds
    }

    fn text(&self) -> &str {
        &self.text
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        self.line_range(line_number)
            .map(|(start, end)| unsafe { self.text.get_unchecked(start..end) })
    }

    fn line_metric(&self, line_number: usize) -> Option<LineMetric> {
        self.line_metrics.get(line_number).cloned()
    }

    fn line_count(&self) -> usize {
        self.line_metrics.len()
    }

    // given a point on the screen, return an offset in the text, basically
    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        let line_num = self
            .line_metrics
            .iter()
            .position(|lm| lm.y_offset + lm.height >= point.y)
            // if we're past the last line, use the last line
            .unwrap_or_else(|| self.line_metrics.len().saturating_sub(1));

        let line = match self.unwrap_frame().get_line(line_num) {
            Some(line) => line,
            None => {
                // if we can't find a line we're either an empty string or we're
                // at the newline at eof
                assert!(self.text.is_empty() || util::trailing_nlf(&self.text).is_some());
                return HitTestPoint::new(self.text.len(), false);
            }
        };
        let line_text = self.line_text(line_num).unwrap();
        let metric = &self.line_metrics[line_num];
        let x_offset = self.x_offsets[line_num];
        // a y position inside this line
        let fake_y = metric.y_offset + metric.baseline;
        // map that back into our inverted coordinate space
        let fake_y = -(self.frame_size.height - fake_y);
        let point_in_string_space = CGPoint::new(point.x - x_offset, fake_y);
        let offset_utf16 = line.get_string_index_for_position(point_in_string_space);
        let mut offset = match offset_utf16 {
            // this is 'kCFNotFound'.
            -1 => self.text.len(),
            n if n >= 0 => {
                let utf16_range = line.get_string_range();
                let rel_offset = (n - utf16_range.location) as usize;
                metric.start_offset
                    + util::count_until_utf16(line_text, rel_offset)
                        .unwrap_or_else(|| line_text.len())
            }
            // some other value; should never happen
            _ => panic!("gross violation of api contract"),
        };

        // if the offset is EOL && EOL is a newline, return the preceding offset
        if offset == metric.end_offset {
            offset -= util::trailing_nlf(line_text).unwrap_or(0);
        };

        let typo_bounds = line.get_typographic_bounds();
        let is_inside_y = point.y >= 0. && point.y <= self.frame_size.height;
        let is_inside_x =
            point_in_string_space.x >= 0. && point_in_string_space.x <= typo_bounds.width;
        let is_inside = is_inside_x && is_inside_y;

        HitTestPoint::new(offset, is_inside)
    }

    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition {
        let idx = idx.min(self.text.len());
        assert!(self.text.is_char_boundary(idx));

        let line_num = self.line_number_for_utf8_offset(idx);
        let line = match self.unwrap_frame().get_line(line_num) {
            Some(line) => line,
            None => {
                assert!(self.text.is_empty() || util::trailing_nlf(&self.text).is_some());
                let lm = &self.line_metrics[line_num];
                let y_pos = lm.y_offset + lm.baseline;
                return HitTestPosition::new(Point::new(0., y_pos), line_num);
            }
        };

        let text = self.line_text(line_num).unwrap();
        let metric = &self.line_metrics[line_num];
        let x_offset = self.x_offsets[line_num];

        let offset_remainder = idx - metric.start_offset;
        let off16: usize = util::count_utf16(&text[..offset_remainder]);
        let line_range = line.get_string_range();
        let char_idx = line_range.location + off16 as isize;
        let x_pos = line.get_offset_for_string_index(char_idx) + x_offset;
        let y_pos = metric.y_offset + metric.baseline;
        HitTestPosition::new(Point::new(x_pos, y_pos), line_num)
    }
}

impl CoreGraphicsTextLayout {
    fn new(
        text: Rc<dyn TextStorage>,
        attr_string: AttributedString,
        width_constraint: f64,
        default_baseline: f64,
        default_line_height: f64,
    ) -> Self {
        let framesetter = Framesetter::new(&attr_string);

        let mut layout = CoreGraphicsTextLayout {
            text,
            attr_string,
            framesetter,
            // all of this is correctly set in `update_width` below
            frame: None,
            frame_size: Size::ZERO,
            bonus_height: 0.0,
            image_bounds: Rect::ZERO,
            // NaN to ensure we always execute code in update_width
            width_constraint: f64::NAN,
            default_baseline,
            default_line_height,
            line_metrics: Rc::new([]),
            x_offsets: Rc::new([]),
            trailing_ws_width: 0.0,
        };
        layout.update_width(width_constraint);
        layout
    }

    // this used to be part of the TextLayout trait; see https://github.com/linebender/piet/issues/298
    #[allow(clippy::float_cmp)]
    fn update_width(&mut self, new_width: impl Into<Option<f64>>) {
        let width = new_width.into().unwrap_or(MAX_LAYOUT_CONSTRAINT);
        let width = if width.is_normal() {
            width
        } else {
            MAX_LAYOUT_CONSTRAINT
        };

        if width.ceil() == self.width_constraint.ceil() {
            return;
        }

        let constraints = CGSize::new(width as CGFloat, MAX_LAYOUT_CONSTRAINT);
        let char_range = self.attr_string.range();
        let rect = CGRect::new(&CGPoint::new(0.0, 0.0), &constraints);
        let path = CGPath::from_rect(rect, None);
        self.width_constraint = width;

        let frame = self.framesetter.create_frame(char_range, &path);
        let layout_metrics = build_line_metrics(
            &frame,
            &self.text,
            self.default_line_height,
            self.default_baseline,
        );
        self.line_metrics = layout_metrics.line_metrics.into();
        self.x_offsets = layout_metrics.x_offsets.into();
        self.trailing_ws_width = layout_metrics.trailing_whitespace;
        self.frame_size = layout_metrics.layout_size;
        assert!(self.line_metrics.len() > 0);

        self.bonus_height = if self.text.is_empty() || util::trailing_nlf(&self.text).is_some() {
            self.line_metrics.last().unwrap().height
        } else {
            0.0
        };

        let mut line_bounds = frame
            .lines()
            .iter()
            .map(Line::get_image_bounds)
            .zip(self.line_metrics.iter().map(|l| l.y_offset + l.baseline))
            // these are relative to the baseline *and* upside down, so we invert y
            .map(|(rect, y_pos)| Rect::new(rect.x0, y_pos - rect.y1, rect.x1, y_pos - rect.y0));

        let first_line_bounds = line_bounds.next().unwrap_or_default();
        self.image_bounds = line_bounds.fold(first_line_bounds, |acc, el| acc.union(el));
        self.frame = Some(frame);
    }

    pub(crate) fn draw(&self, ctx: &mut CGContextRef) {
        let lines = self.unwrap_frame().lines();
        let lines_len = lines.len();
        assert!(self.x_offsets.len() >= lines_len);
        assert!(self.line_metrics.len() >= lines_len);

        for (i, line) in lines.iter().enumerate() {
            let x = self.x_offsets.get(i).copied().unwrap_or_default();
            // because coretext has an inverted coordinate system we have to manually flip lines
            let y_off = self
                .line_metrics
                .get(i)
                .map(|lm| lm.y_offset + lm.baseline)
                .unwrap_or_default();
            let y = self.frame_size.height - y_off;
            ctx.set_text_position(x, y);
            line.draw(ctx)
        }
    }

    #[inline]
    fn unwrap_frame(&self) -> &Frame {
        self.frame.as_ref().expect("always inited in ::new")
    }

    fn line_number_for_utf8_offset(&self, offset: usize) -> usize {
        match self
            .line_metrics
            .binary_search_by_key(&offset, |lm| lm.start_offset)
        {
            Ok(line) => line,
            Err(line) => line.saturating_sub(1),
        }
    }

    fn line_range(&self, line: usize) -> Option<(usize, usize)> {
        self.line_metrics
            .get(line)
            .map(|lm| (lm.start_offset, lm.end_offset))
    }

    #[allow(dead_code)]
    fn debug_print_lines(&self) {
        for (i, lm) in self.line_metrics.iter().enumerate() {
            let range = lm.range();
            println!(
                "L{} ({}..{}): '{}'",
                i,
                range.start,
                range.end,
                &self.text[lm.range()].escape_debug()
            );
        }
    }
}

struct LayoutMetrics {
    line_metrics: Vec<LineMetric>,
    trailing_whitespace: f64,
    x_offsets: Vec<f64>,
    layout_size: Size,
}

/// Returns metrics, x_offsets, and the max width including trailing whitespace.
#[allow(clippy::while_let_on_iterator)]
fn build_line_metrics(
    frame: &Frame,
    text: &str,
    default_line_height: f64,
    default_baseline: f64,
) -> LayoutMetrics {
    let line_origins = frame.get_line_origins(CFRange::init(0, 0));
    assert_eq!(frame.lines().len(), line_origins.len());

    let mut metrics = Vec::with_capacity(frame.lines().len() + 1);
    let mut x_offsets = Vec::with_capacity(frame.lines().len() + 1);
    let mut cumulative_height = 0.0;
    let mut max_width = 0f64;
    let mut max_width_with_ws = 0f64;

    let mut chars = text.chars();
    let mut cur_16 = 0;
    let mut cur_8 = 0;

    // a closure for converting our offsets
    let mut utf16_to_utf8 = |off_16| {
        if off_16 == 0 {
            0
        } else {
            while let Some(c) = chars.next() {
                cur_16 += c.len_utf16();
                cur_8 += c.len_utf8();
                if cur_16 == off_16 {
                    return cur_8;
                }
            }
            panic!("error calculating utf8 offsets");
        }
    };

    let mut last_line_end = 0;
    for (i, line) in frame.lines().iter().enumerate() {
        let range = line.get_string_range();

        let start_offset = last_line_end;
        let end_offset = utf16_to_utf8((range.location + range.length) as usize);
        last_line_end = end_offset;

        let trailing_whitespace = count_trailing_ws(&text[start_offset..end_offset]);

        let ws_width = line.get_trailing_whitespace_width();
        let typo_bounds = line.get_typographic_bounds();
        max_width_with_ws = max_width_with_ws.max(typo_bounds.width);
        max_width = max_width.max(typo_bounds.width - ws_width);

        let baseline = (typo_bounds.ascent + 0.5).floor();
        let height =
            compute_line_height(typo_bounds.ascent, typo_bounds.descent, typo_bounds.leading);
        let y_offset = cumulative_height;
        cumulative_height += height;

        metrics.push(LineMetric {
            start_offset,
            end_offset,
            trailing_whitespace,
            baseline,
            height,
            y_offset,
        });
        x_offsets.push(line_origins[i].x);
    }

    // adjust our x_offsets so that we zero leading whitespace (relevant if right-aligned)
    let min_x_offset = if x_offsets.is_empty() {
        0.0
    } else {
        x_offsets
            .iter()
            .fold(f64::MAX, |mx, this| if *this < mx { *this } else { mx })
    };
    x_offsets.iter_mut().for_each(|off| *off -= min_x_offset);

    // empty string is treated as a single empty line
    if text.is_empty() {
        metrics.push(LineMetric {
            height: default_line_height,
            baseline: default_baseline,
            ..Default::default()
        });
    // newline at EOF is treated as an additional empty line
    } else if util::trailing_nlf(text).is_some() {
        let newline_eof = metrics
            .last()
            .map(|lm| {
                LineMetric {
                    start_offset: text.len(),
                    end_offset: text.len(),
                    // use height and baseline of preceding line; more likely
                    // to be correct than the default.
                    // FIXME: for this to be actually correct we would need the metrics
                    // of the font used in the line's last run
                    height: lm.height,
                    baseline: lm.baseline,
                    y_offset: lm.y_offset + lm.height,
                    trailing_whitespace: 0,
                }
            })
            .unwrap();
        let x_offset = x_offsets.last().copied().unwrap();
        metrics.push(newline_eof);
        x_offsets.push(x_offset);
    }

    let layout_size = Size::new(max_width, cumulative_height);

    LayoutMetrics {
        line_metrics: metrics,
        x_offsets,
        layout_size,
        trailing_whitespace: max_width_with_ws,
    }
}

// this may not be exactly right, but i'm also not sure we ever use this?
// see https://stackoverflow.com/questions/5511830/how-does-line-spacing-work-in-core-text-and-why-is-it-different-from-nslayoutm
fn compute_line_height(ascent: f64, descent: f64, leading: f64) -> f64 {
    let leading = leading.max(0.0);
    let leading = (leading + 0.5).floor();
    leading + (descent + 0.5).floor() + (ascent + 0.5).floor()
    // in the link they also calculate an ascender delta that is used to adjust line
    // spacing in some cases, but this feels finicky and we can choose not to do it.
}

fn count_trailing_ws(s: &str) -> usize {
    //FIXME: this is just ascii whitespace
    s.as_bytes()
        .iter()
        .rev()
        .take_while(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
        .count()
}

/// Generate an opentype tag. The string should be exactly 4 bytes long.
///
/// ```no_compile
/// const WEIGHT_AXIS = make_opentype_tag("wght");
/// ```
const fn make_opentype_tag(raw: &str) -> u32 {
    let b = raw.as_bytes();
    ((b[0] as u32) << 24) | ((b[1] as u32) << 16) | ((b[2] as u32) << 8) | (b[3] as u32)
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
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
    fn line_offsets() {
        let text = "hi\ni'm\n😀 four\nlines";
        let a_font = FontFamily::new_unchecked("Helvetica");
        let layout = CoreGraphicsTextLayoutBuilder::new(text)
            .font(a_font, 16.0)
            .build()
            .unwrap();
        assert_eq!(layout.line_text(0), Some("hi\n"));
        assert_eq!(layout.line_text(1), Some("i'm\n"));
        assert_eq!(layout.line_text(2), Some("😀 four\n"));
        assert_eq!(layout.line_text(3), Some("lines"));
    }

    #[test]
    fn metrics() {
        let text = "🤡:\na string\nwith a number \n of lines";
        let a_font = FontFamily::new_unchecked("Helvetica");
        let layout = CoreGraphicsTextLayoutBuilder::new(text)
            .font(a_font, 16.0)
            .build()
            .unwrap();

        let line1 = layout.line_metric(0).unwrap();
        assert_eq!(line1.range(), 0..6);
        assert_eq!(line1.trailing_whitespace, 1);
        layout.line_metric(1);

        let line3 = layout.line_metric(2).unwrap();
        assert_eq!(line3.range(), 15..30);
        assert_eq!(line3.trailing_whitespace, 2);

        let line4 = layout.line_metric(3).unwrap();
        assert_eq!(layout.line_text(3), Some(" of lines"));
        assert_eq!(line4.trailing_whitespace, 0);

        let total_height = layout.frame_size.height;
        assert_eq!(line4.y_offset + line4.height, total_height);

        assert!(layout.line_metric(4).is_none());
    }

    // test that at least we're landing on the correct line
    #[test]
    fn basic_hit_testing() {
        let text = "1\n😀\n8\nA";
        let a_font = FontFamily::new_unchecked("Helvetica");
        let layout = CoreGraphicsTextLayoutBuilder::new(text)
            .font(a_font, 16.0)
            .build()
            .unwrap();

        assert_eq!(layout.line_count(), 4);

        let p1 = layout.hit_test_point(Point::ZERO);
        assert_eq!(p1.idx, 0);
        assert!(p1.is_inside);
        let p2 = layout.hit_test_point(Point::new(2.0, 15.9));
        assert_eq!(p2.idx, 0);
        assert!(p2.is_inside);

        let p3 = layout.hit_test_point(Point::new(50.0, 10.0));
        assert_eq!(p3.idx, 1);
        assert!(!p3.is_inside);

        let p4 = layout.hit_test_point(Point::new(4.0, 25.0));
        assert_eq!(p4.idx, 2);
        assert!(p4.is_inside);

        let p5 = layout.hit_test_point(Point::new(2.0, 64.0));
        assert_eq!(p5.idx, 9);
        assert!(p5.is_inside);

        let p6 = layout.hit_test_point(Point::new(10.0, 64.0));
        assert_eq!(p6.idx, 10);
        assert!(p6.is_inside);
    }

    #[test]
    fn hit_test_end_of_single_line() {
        let text = "hello";
        let a_font = FontFamily::new_unchecked("Helvetica");
        let layout = CoreGraphicsTextLayoutBuilder::new(text)
            .font(a_font, 16.0)
            .build()
            .unwrap();
        let pt = layout.hit_test_point(Point::new(0.0, 5.0));
        assert_eq!(pt.idx, 0);
        assert!(pt.is_inside);
        let next_to_last = layout.frame_size.width - 10.0;
        let pt = layout.hit_test_point(Point::new(next_to_last, 0.0));
        assert_eq!(pt.idx, 4);
        assert!(pt.is_inside);
        let pt = layout.hit_test_point(Point::new(100.0, 5.0));
        assert_eq!(pt.idx, 5);
        assert!(!pt.is_inside);
    }

    #[test]
    fn hit_test_empty_string() {
        let a_font = FontFamily::new_unchecked("Helvetica");
        let layout = CoreGraphicsTextLayoutBuilder::new("")
            .font(a_font, 12.0)
            .build()
            .unwrap();
        let pt = layout.hit_test_point(Point::new(0.0, 0.0));
        assert_eq!(pt.idx, 0);
        let pos = layout.hit_test_text_position(0);
        assert_eq!(pos.point.x, 0.0);
        assert_close!(pos.point.y, 10.0, 3.0);
        let line = layout.line_metric(0).unwrap();
        assert_close!(line.height, 12.0, 3.0);
    }

    #[test]
    fn hit_test_text_position() {
        let text = "aaaaa\nbbbbb";
        let a_font = FontFamily::new_unchecked("Helvetica");
        let layout = CoreGraphicsTextLayoutBuilder::new(text)
            .font(a_font, 16.0)
            .build()
            .unwrap();
        let p1 = layout.hit_test_text_position(0);
        assert_close!(p1.point.y, 12.0, 0.5);

        let p1 = layout.hit_test_text_position(7);
        assert_close!(p1.point.y, 28.0, 0.5);
        // just the general idea that this is the second character
        assert_close!(p1.point.x, 10.0, 5.0);
    }

    #[test]
    fn hit_test_text_position_astral_plane() {
        let text = "👾🤠\n🤖🎃👾";
        let a_font = FontFamily::new_unchecked("Helvetica");
        let layout = CoreGraphicsTextLayoutBuilder::new(text)
            .font(a_font, 16.0)
            .build()
            .unwrap();
        let p0 = layout.hit_test_text_position(4);
        let p1 = layout.hit_test_text_position(8);
        let p2 = layout.hit_test_text_position(13);

        assert!(p1.point.x > p0.point.x);
        assert!(p1.point.y == p0.point.y);
        assert!(p2.point.y > p1.point.y);
    }

    #[test]
    fn missing_font_is_missing() {
        assert!(CoreGraphicsText::new_with_unique_state()
            .font_family("Segoe UI")
            .is_none());
    }

    #[test]
    fn line_text_empty_string() {
        let layout = CoreGraphicsTextLayoutBuilder::new("").build().unwrap();
        assert_eq!(layout.line_text(0), Some(""));
    }

    /// Trailing whitespace should all be included in the text of the line,
    /// and should be reported in the `trailing_whitespace` field of the line metrics.
    #[test]
    fn line_test_tabs() {
        let line_text = "a\t\t\t\t\n";
        let layout = CoreGraphicsTextLayoutBuilder::new(line_text)
            .build()
            .unwrap();
        assert_eq!(layout.line_count(), 2);
        assert_eq!(layout.line_text(0), Some(line_text));
        let metrics = layout.line_metric(0).unwrap();
        assert_eq!(metrics.trailing_whitespace, line_text.len() - 1);
    }
}
