//! Traits for fonts and text handling.

use std::ops::{Range, RangeBounds};
use std::sync::Arc;

use crate::kurbo::{Point, Rect, Size};
use crate::Error;

/// The piet text API.
///
/// This trait is the interface for text-related functionality, such as font
/// management and text layout.
pub trait Text: Clone {
    type TextLayoutBuilder: TextLayoutBuilder<Out = Self::TextLayout>;
    type TextLayout: TextLayout;
    /// Query the platform for a font with a given name, and return a [`FontFamily`]
    /// object corresponding to that font, if it is found.
    ///
    /// # Examples
    ///
    /// Trying a preferred font, falling back if it isn't found.
    ///
    /// ```
    /// # use piet::*;
    /// # let mut ctx = NullRenderContext::new();
    /// # let text = ctx.text();
    /// let text_font = text.font_family("Charter")
    ///     .or_else(|| text.font_family("Garamond"))
    ///     .unwrap_or(FontFamily::SERIF);
    /// ```
    ///
    /// [`FontFamily`]: struct.FontFamily.html
    fn font_family(&mut self, family_name: &str) -> Option<FontFamily>;

    /// Load the provided font data and make it available for use.
    ///
    /// This method takes font data (such as the contents of a file on disk) and
    /// attempts to load it, making it subsequently available for use.
    ///
    /// If loading is successful, this method will return a [`FontFamily`] handle
    /// that can be used to select this font when constructing a [`TextLayout`].
    ///
    /// # Notes
    ///
    /// ## font familes and styles:
    ///
    /// If you wish to use multiple fonts in a given family, you will need to
    /// load them individually. This method will return the same handle for
    /// each font in the same family; the handle **does not refer to a specific
    /// font**. This means that if you load bold and regular fonts from the
    /// same family, to *use* the bold version you must, when constructing your
    /// [`TextLayout`], pass the family as well as the correct weight.
    ///
    /// *If you wish to use custom fonts, load each concrete instance of the
    /// font-family that you wish to use; that is, if you are using regular,
    /// bold, italic, and bold-italic, you should be loading four distinct fonts.*
    ///
    /// ## family name masking
    ///
    /// If you load a custom font, the family name of your custom font will take
    /// precedence over system familes of the same name; so your 'Helvetica' will
    /// potentially interfere with the use of the platform 'Helvetica'.
    ///
    /// # Examples
    ///
    /// ```
    /// # use piet::*;
    /// # let mut ctx = NullRenderContext::new();
    /// # let text = ctx.text();
    /// # fn get_font_data(name: &str) -> Vec<u8> { Vec::new() }
    /// let helvetica_regular = get_font_data("Helvetica-Regular");
    /// let helvetica_bold = get_font_data("Helvetica-Bold");
    ///
    /// let regular = text.load_font(&helvetica_regular).unwrap();
    /// let bold = text.load_font(&helvetica_bold).unwrap();
    /// assert_eq!(regular, bold);
    ///
    /// let layout = text.new_text_layout("Custom Fonts")
    ///     .font(regular, 12.0)
    ///     .range_attribute(6.., FontWeight::BOLD);
    ///
    /// ```
    ///
    /// [`TextLayout`]: trait.TextLayout.html
    /// [`FontFamily`]: struct.FontFamily.html
    fn load_font(&mut self, data: &[u8]) -> Result<FontFamily, Error>;

    /// Create a new layout object to display the provided `text`.
    ///
    /// The returned object is a [`TextLayoutBuilder`]; methods on that type
    /// can be used to customize the layout.
    ///
    /// [`TextLayoutBuilder`]: trait.TextLayoutBuilder.html
    fn new_text_layout(&mut self, text: &str) -> Self::TextLayoutBuilder;
}

/// A reference to a font family.
///
/// This may be either a CSS-style "generic family name", such as "serif"
/// or "monospace", or it can be an explicit family name.
///
/// To use a generic family name, use the provided associated constants:
/// `FontFamily::SERIF`, `FontFamily::SANS_SERIF`, `FontFamily::SYSTEM_UI`,
/// and `FontFamily::MONOSPACE`.
///
/// To use a specific font family you should not construct this type directly;
/// instead you should verify that the desired family exists, via the
/// [`Text::font`] API.
///
/// [`Text::font`]: trait.Text.html#tymethod.font
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFamily(FontFamilyInner);

/// The inner representation of a font family.
///
/// This is not public API for users of piet; it is exposed for backends only.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[doc(hidden)]
#[non_exhaustive]
pub enum FontFamilyInner {
    Serif,
    SansSerif,
    Monospace,
    SystemUi,
    Named(Arc<str>),
}

impl FontFamily {
    /// A san-serif font, such as Arial or Helvetica.
    pub const SANS_SERIF: FontFamily = FontFamily(FontFamilyInner::SansSerif);
    /// A serif font, such as Times New Roman or Charter.
    pub const SERIF: FontFamily = FontFamily(FontFamilyInner::Serif);
    /// The platform's preferred UI font; San Francisco on macOS, and Segoe UI
    /// on recent Windows.
    pub const SYSTEM_UI: FontFamily = FontFamily(FontFamilyInner::SystemUi);
    /// A monospace font.
    pub const MONOSPACE: FontFamily = FontFamily(FontFamilyInner::Monospace);

    /// TODO: document me: this should generally not be used, instead get your
    /// font from the text system, or use one of the consts.
    pub fn new_unchecked(s: impl Into<Arc<str>>) -> Self {
        FontFamily(FontFamilyInner::Named(s.into()))
    }

    pub fn name(&self) -> &str {
        match &self.0 {
            FontFamilyInner::Serif => "serif",
            FontFamilyInner::SansSerif => "sans-serif",
            FontFamilyInner::SystemUi => "system-ui",
            FontFamilyInner::Monospace => "monospace",
            FontFamilyInner::Named(s) => &s,
        }
    }

    /// Returns `true` if this is a generic font family.
    pub fn is_generic(&self) -> bool {
        !matches!(self.0, FontFamilyInner::Named(_))
    }

    /// Backend-only API; access the inner `FontFamilyInner` enum.
    #[doc(hidden)]
    pub fn inner(&self) -> &FontFamilyInner {
        &self.0
    }
}

impl Default for FontFamily {
    fn default() -> Self {
        FontFamily::SYSTEM_UI
    }
}

/// A font weight, represented as a value in the range 1..=1000.
///
/// This is based on the [CSS `font-weight`] property. In general, you should
/// prefer the constants defined on this type, such as `FontWeight::REGULAR` or
/// `FontWeight::BOLD`.
///
/// [CSS `font-weight`]: https://developer.mozilla.org/en-US/docs/Web/CSS/font-weight
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontWeight(u16);

impl FontWeight {
    pub const THIN: FontWeight = FontWeight(100);
    pub const HAIRLINE: FontWeight = FontWeight::THIN;

    pub const EXTRA_LIGHT: FontWeight = FontWeight(200);

    pub const LIGHT: FontWeight = FontWeight(300);

    pub const REGULAR: FontWeight = FontWeight(400);
    pub const NORMAL: FontWeight = FontWeight::REGULAR;

    pub const MEDIUM: FontWeight = FontWeight(500);

    pub const SEMI_BOLD: FontWeight = FontWeight(600);

    pub const BOLD: FontWeight = FontWeight(700);

    pub const EXTRA_BOLD: FontWeight = FontWeight(800);

    pub const BLACK: FontWeight = FontWeight(900);
    pub const HEAVY: FontWeight = FontWeight::BLACK;

    pub const EXTRA_BLACK: FontWeight = FontWeight(950);

    /// Create a new `FontWeight` with a custom value.
    ///
    /// Values will be clamped to the range 1..=1000.
    pub fn new(raw: u16) -> FontWeight {
        let raw = raw.min(1000).max(1);
        FontWeight(raw)
    }

    /// Return the raw value as a u16.
    pub const fn to_raw(self) -> u16 {
        self.0
    }
}

/// Attributes that can be applied to text.
pub enum TextAttribute {
    /// The font family.
    Font(FontFamily),
    /// The font size, in points.
    Size(f64),
    /// The [`FontWeight`](struct.FontWeight.html).
    Weight(FontWeight),
    /// The foreground color of the text.
    ForegroundColor(crate::Color),
    //BackgroundColor(crate::Color),
    /// Italics.
    Italic(bool),
    /// Underline.
    Underline(bool),
}

/// A trait for laying out text.
pub trait TextLayoutBuilder: Sized {
    type Out: TextLayout;

    /// Set a max width for this layout.
    ///
    /// You may pass an `f64` to this method to indicate a width (in display points)
    /// that will be used for word-wrapping.
    ///
    /// If you pass `f64::INFINITY`, words will not be wrapped; this is the
    /// default behaviour.
    fn max_width(self, width: f64) -> Self;

    /// Set the [`TextAlignment`] to be used for this layout.
    ///
    /// [`TextAlignment`]: enum.TextAlignment.html
    fn alignment(self, alignment: TextAlignment) -> Self;

    /// A convenience method for setting the default font family and size.
    ///
    /// # Examples
    ///
    /// ```
    /// # use piet::*;
    /// # let mut ctx = NullRenderContext::new();
    /// # let mut text = ctx.text();
    ///
    /// let times = text.font_family("Times New Roman").unwrap();
    ///
    /// // the following are equivalent
    /// let layout_one = text.new_text_layout("hello everyone!")
    ///     .font(times.clone(), 12.0)
    ///     .build();
    ///
    /// let layout_two = text.new_text_layout("hello everyone!")
    ///     .default_attribute(TextAttribute::Font(times.clone()))
    ///     .default_attribute(TextAttribute::Size(12.0))
    ///     .build();
    /// ```
    fn font(self, font: FontFamily, font_size: f64) -> Self {
        self.default_attribute(TextAttribute::Font(font))
            .default_attribute(TextAttribute::Size(font_size))
    }

    /// Add a default [`TextAttribute`] for this layout.
    ///
    /// Default attributes will be used for regions of the layout that do not
    /// have explicit attributes added via [`range_attribute`].
    ///
    /// You must set default attributes before setting range attributes,
    /// or the implementation is free to ignore them.
    ///
    /// [`TextAttribute`]: enum.TextAttribute.html
    /// [`range_attribute`]: #tymethod.range_attribute
    fn default_attribute(self, attribute: impl Into<TextAttribute>) -> Self;

    /// Add a [`TextAttribute`] to a range of this layout.
    ///
    /// The `range` argument is can be any of the range forms accepted by
    /// slice indexing, such as `..`, `..n`, `n..`, `n..m`, etcetera.
    ///
    /// The `attribute` argument is a [`TextAttribute`] or any type that can be
    /// converted to such an attribute; for instance you may pass a [`FontWeight`]
    /// directly.
    ///
    /// ## Notes
    ///
    /// This is a low-level API; what this means in particular is that it is designed
    /// to be efficiently implemented, not necessarily ergonomic to use, and there
    /// may be a few gotchas.
    ///
    /// **ranges of added attributes should be added in non-decreasing start order**.
    /// This is to say that attributes should be added in the order of the start
    /// of their ranges. Attributes added out of order may be skipped.
    ///
    /// **attributes do not stack**. Setting the range `0..100` to `FontWeight::BOLD`
    /// and then setting the range `20..50` to `FontWeight::THIN` will result in
    /// the range `50..100` being reset to the default font weight; we will not
    /// remember that you had earlier set it to `BOLD`.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use piet::*;
    /// # let mut ctx = NullRenderContext::new();
    /// # let mut text = ctx.text();
    ///
    /// let times = text.font_family("Times New Roman").unwrap();
    /// let layout = text.new_text_layout("This API is okay, I guess?")
    ///     .font(FontFamily::MONOSPACE, 12.0)
    ///     .default_attribute(TextAttribute::Italic(true))
    ///     .range_attribute(..5, FontWeight::BOLD)
    ///     .range_attribute(5..14, times)
    ///     .range_attribute(20.., TextAttribute::ForegroundColor(Color::rgb(1.0, 0., 0.,)))
    ///     .build();
    /// ```
    ///
    /// [`TextAttribute`]: enum.TextAttribute.html
    /// [`FontWeight`]: struct.FontWeight.html
    fn range_attribute(
        self,
        range: impl RangeBounds<usize>,
        attribute: impl Into<TextAttribute>,
    ) -> Self;

    fn build(self) -> Result<Self::Out, Error>;
}

/// The alignment of text in a [`TextLayout`].
///
/// [`TextLayout`]: trait.TextLayout.html
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    /// Text is aligned to the left edge in left-to-right scripts, and the
    /// right edge in right-to-left scripts.
    Start,
    /// Text is aligned to the right edge in left-to-right scripts, and the
    /// left edge in right-to-left scripts.
    End,
    Center,
    Justified,
}

/// A drawable text object.
///
/// ## Line Breaks
///
/// A text layout may be broken into multiple lines in order to fit within a given width. Line breaking is generally done
/// between words (whitespace-separated).
///
/// When resizing the width of the text layout, calling [`update_width`][] on the text layout will
/// recalculate line breaks and modify in-place.
///
/// A line's text and [`LineMetric`][]s can be accessed line-by-line by 0-indexed line number.
///
/// Fields on ['LineMetric`] include:
/// - line start offset from text layout beginning (in UTF-8 code units)
/// - line end offset from text layout beginning (in UTF-8 code units)
/// - line trailing whitespace (in UTF-8 code units)
/// - line's baseline, distance of the baseline from the top of the line
/// - line height
///
/// The trailing whitespace distinction is important. Lines are broken at the grapheme boundary after
/// whitespace, but that whitepace is not necessarily rendered since it's just the trailing
/// whitepace at the end of a line. Keeping the trailing whitespace data available allows API
/// consumers to determine their own trailing whitespace strategy.
///
/// ## Text Position
///
/// A text position is the offset in the underlying string, defined in utf-8 code units, as is standard for Rust strings.
///
/// However, text position is also related to valid cursor positions. Therefore:
/// - The beginning of a line has text position `0`.
/// - The end of a line is a valid text position. e.g. `text.len()` is a valid text position.
/// - If the text position is not at a code point or grapheme boundary, undesirable behavior may
/// occur.
///
/// [`update_width`]: trait.TextLayout.html#tymethod.update_width
/// [`LineMetric`]: struct.LineMetric.html
///
pub trait TextLayout: Clone {
    /// Measure the advance width of the text.
    #[deprecated(since = "0.2.0", note = "Use size().width insead")]
    fn width(&self) -> f64;

    /// The total size of this `TextLayout`.
    ///
    /// This is the size required to draw this `TextLayout`, as provided by the
    /// platform text system.
    ///
    /// # Note
    ///
    /// This is not currently defined very rigorously; in particular we do not
    /// specify whether this should include half-leading or paragraph spacing
    /// above or below the text.
    ///
    /// We would ultimately like to review and attempt to standardize this
    /// behaviour, but it is out of scope for the time being.
    fn size(&self) -> Size;

    /// Returns a `Rect` representing the bounding box of the glyphs in this layout,
    /// relative to the top-left of the layout object.
    ///
    /// This is sometimes called the bounding box or the inking rect.
    fn image_bounds(&self) -> Rect;

    /// The text used to create this layout.
    fn text(&self) -> &str;

    /// Change the width of this `TextLayout`.
    ///
    /// This may be an `f64`, or `None` if this layout is not constrained;
    /// `None` is equivalent to `std::f64::INFINITY`.
    fn update_width(&mut self, new_width: impl Into<Option<f64>>) -> Result<(), Error>;

    /// Given a line number, return a reference to that line's underlying string.
    fn line_text(&self, line_number: usize) -> Option<&str>;

    /// Given a line number, return a reference to that line's metrics.
    fn line_metric(&self, line_number: usize) -> Option<LineMetric>;

    /// Returns total number of lines in the text layout.
    fn line_count(&self) -> usize;

    /// Given a `Point`, determine the corresponding text position.
    ///
    /// This is used for things like mapping a mouse click to a cursor position.
    ///
    /// The point should be in the coordinate space of the layout object.
    ///
    /// ## Return value:
    /// Returns a [`HitTestPoint`][] describing the results of the test.
    ///
    /// The [`HitTestPoint`][] field `is_inside` is true if the tested point
    /// falls within the bounds of the text, `false` otherwise.
    ///
    /// The [`HitTestPoint`][] field `idx` is the index, in the string used to
    /// create this [`TextLayout`][], of the start of the grapheme cluster
    /// closest to the tested point.
    ///
    /// ## Notes:
    ///
    /// This will always return *some* text position. If the point is outside of
    /// the bounds of the layout, it will return the nearest text position.
    ///
    /// For more on text positions, see docs for the [`TextLayout`] trait.
    ///
    /// [`HitTestPoint`]: struct.HitTestPoint.html
    /// [`TextLayout`]: ../piet/trait.TextLayout.html
    fn hit_test_point(&self, point: Point) -> HitTestPoint;

    /// Given a grapheme boundary in the string used to create this [`TextLayout`],
    /// return information about the location of that boundary within the layout
    /// object.
    ///
    ///
    /// ## Return value:
    /// Returns a [`HitTestPosition`][] struct describing the results of the test.
    ///
    /// The [`HitTestPosition`][] field `point` is a `Point`, on the baseline
    /// of the line containing this grapheme cluster, of the grapheme's leading edge,
    /// relative to the origin of the layout object.
    ///
    /// ## Panics:
    ///
    // FIXME: behaviour currently differs between backends, but some backends panic.
    // decide whether this panics or not, and standardize backend behaviour.
    /// This method may panic if the text position is not a codepoint boundary,
    /// or if it is greater than the length of the text.
    ///
    /// For more on text positions, see docs for the [`TextLayout`] trait.
    ///
    /// [`HitTestPosition`]: struct.HitTestPosition.html
    /// [`TextLayout`]: ../piet/trait.TextLayout.html
    //FIXME: under what circumstances should this return `None`? A reasonable
    //case would be when trimming has caused an index to not be included in the
    //layout's text?`
    fn hit_test_text_position(&self, idx: usize) -> Option<HitTestPosition>;

    /// Returns a vector of `Rect`s that cover the region of the text indicated
    /// by `range`.
    ///
    /// The returned rectangles are suitable for things like drawing selection
    /// regions or highlights.
    ///
    /// `range` will be clamped to the length of the text if necessary.
    ///
    /// Note: this implementation is not currently BiDi aware; it will be updated
    /// when BiDi support is added.
    fn rects_for_range(&self, range: impl RangeBounds<usize>) -> Vec<Rect> {
        let text_len = self.text().len();
        let mut range = crate::util::resolve_range(range, text_len);
        range.start = range.start.min(text_len);
        range.end = range.end.min(text_len);

        let first_line = self.hit_test_text_position(range.start).unwrap().line;
        let last_line = self.hit_test_text_position(range.end).unwrap().line;

        let mut result = Vec::new();

        for line in first_line..=last_line {
            let metrics = self.line_metric(line).unwrap();
            let y0 = metrics.y_offset;
            let y1 = y0 + metrics.height;
            let line_range_start = if line == first_line {
                range.start
            } else {
                metrics.start_offset
            };

            let line_range_end = if line == last_line {
                range.end
            } else {
                metrics.end_offset - metrics.trailing_whitespace
            };
            let start_point = self.hit_test_text_position(line_range_start).unwrap();
            let end_point = self.hit_test_text_position(line_range_end).unwrap();
            result.push(Rect::new(start_point.point.x, y0, end_point.point.x, y1));
        }

        result
    }
}

/// Metadata about each line in a text layout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineMetric {
    /// The start index of this line in the underlying `String` used to create the
    /// [`TextLayout`] to which this line belongs.
    ///
    /// [`TextLayout`]: trait.TextLayout.html
    pub start_offset: usize,

    /// The end index of this line in the underlying `String` used to create the
    /// [`TextLayout`] to which this line belongs.
    ///
    /// This is the end of an exclusive range; this index is not part of the line.
    ///
    /// Includes trailing whitespace.
    ///
    /// [`TextLayout`]: trait.TextLayout.html
    pub end_offset: usize,

    /// The length of the trailing whitespace at the end of this line, in utf-8 code units.
    pub trailing_whitespace: usize,

    /// The distance from the top of the line (`y_offset`) to the baseline.
    pub baseline: f64,

    /// The height of the line.
    ///
    /// This value is intended to be used to determine the height of features
    /// such as cursors and selection regions. Although it is generally the case
    /// that `y_offset + height` for line `n` is equal to the `y_offset` of
    /// line `n + 1`, this is not strictly enforced, and should not be counted on.
    pub height: f64,

    /// The y position of the top of this line, relative to the top of the layout.
    ///
    /// It should be possible to use this position, in conjunction with `height`,
    /// to determine the region that would be used for things like text selection.
    pub y_offset: f64,
}

impl LineMetric {
    /// The utf-8 range in the underlying `String` used to create the
    /// [`TextLayout`] to which this line belongs.
    ///
    /// [`TextLayout`]: trait.TextLayout.html
    #[inline]
    pub fn range(&self) -> Range<usize> {
        self.start_offset..self.end_offset
    }
}

/// Result of hit testing a point in a [`TextLayout`].
///
/// This type is returned by [`TextLayout::hit_test_point`].
///
/// [`TextLayout`]: ../piet/trait.TextLayout.html
/// [`TextLayout::hit_test_point`]: ../piet/trait.TextLayout.html#tymethod.hit_test_point
#[derive(Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct HitTestPoint {
    /// The index representing the grapheme boundary closest to the `Point`.
    pub idx: usize,
    /// Whether or not the point was inside the bounds of the layout object.
    ///
    /// A click outside the layout object will still resolve to a position in the
    /// text; for instance a click to the right edge of a line will resolve to the
    /// end of that line, and a click below the last line will resolve to a
    /// position in that line.
    pub is_inside: bool,
}

/// Result of hit testing a text position in a [`TextLayout`].
///
/// This type is returned by [`TextLayout::hit_test_text_position`].
///
/// [`TextLayout`]: ../piet/trait.TextLayout.html
/// [`TextLayout::hit_test_text_position`]: ../piet/trait.TextLayout.html#tymethod.hit_test_text_position
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct HitTestPosition {
    /// the `point`'s `x` value is the position of the leading edge of the
    /// grapheme cluster containing the text position. The `y` value corresponds
    /// to the baseline of the line containing that grapheme cluster.
    //FIXME: maybe we should communicate more about this position? for instance
    //instead of returning an x/y point, we could return the x offset, the line's y_offset,
    //and the line height (everything tou would need to draw a cursor)
    pub point: Point,
    /// The number of the line containing this position.
    ///
    /// This value can be used to retrieve the [`LineMetric`] for this line,
    /// via the [`TextLayout::line_metric`] method.
    ///
    /// [`LineMetric`]: struct.LineMetric.html
    /// [`TextLayout::line_metric`]: trait.TextLayout.html#tymethod.line_metric
    pub line: usize,
}

impl HitTestPoint {
    /// Only for use by backends
    #[doc(hidden)]
    pub fn new(idx: usize, is_inside: bool) -> HitTestPoint {
        HitTestPoint { idx, is_inside }
    }
}

impl HitTestPosition {
    /// Only for use by backends
    #[doc(hidden)]
    pub fn new(point: Point, line: usize) -> HitTestPosition {
        HitTestPosition { point, line }
    }
}

impl From<FontFamily> for TextAttribute {
    fn from(t: FontFamily) -> TextAttribute {
        TextAttribute::Font(t)
    }
}

impl From<FontWeight> for TextAttribute {
    fn from(src: FontWeight) -> TextAttribute {
        TextAttribute::Weight(src)
    }
}

impl Default for TextAlignment {
    fn default() -> Self {
        TextAlignment::Start
    }
}

impl Default for FontWeight {
    fn default() -> Self {
        FontWeight::REGULAR
    }
}
