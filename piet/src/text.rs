//! Traits for fonts and text handling.

use std::ops::{Range, RangeBounds};

use crate::kurbo::{Point, Rect, Size};
use crate::{Color, Error, FontFamily, FontStyle, FontWeight};

/// The Piet text API.
///
/// This trait is the interface for text-related functionality, such as font
/// management and text layout.
pub trait Text: Clone {
    /// A concrete type that implements the [`TextLayoutBuilder`] trait.
    ///
    /// [`TextLayoutBuilder`]: trait.TextLayoutBuilder.html
    type TextLayoutBuilder: TextLayoutBuilder<Out = Self::TextLayout>;

    /// A concrete type that implements the [`TextLayout`] trait.
    ///
    /// [`TextLayout`]: trait.TextLayout.html
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
    /// ## font families and styles:
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
    /// precedence over system families of the same name; so your 'Helvetica' will
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
    /// The text is a type that impls `Into<Arc<str>>`. This is an optimization;
    /// because the layout object needs to retain the text, if the caller wants
    /// to avoid duplicate data they can use an `Arc`. If this doesn't matter,
    /// they can pass a `&str`, which the layout will retain.
    ///
    /// [`TextLayoutBuilder`]: trait.TextLayoutBuilder.html
    fn new_text_layout(&mut self, text: impl TextStorage) -> Self::TextLayoutBuilder;
}

/// A type that stores text.
///
/// This allows the client to more completely control how text is stored.
/// If you do not care about this, implementations are provided for `String`,
/// `Arc<str>`, and `Rc<str>`.
///
/// This has a `'static` bound because the inner type will be behind a shared
/// pointer.
///
/// # Implementors
///
/// This trait expects immutable data. Mutating the data (using interior mutability)
/// May cause any [`TextLayout`] objects using this [`TextStorage`] to become
/// inconsistent.
pub trait TextStorage: 'static {
    /// Return the underlying text as a contiguous buffer.
    ///
    /// Types that do not store their text as a contiguous buffer (such as ropes
    /// or gap buffers) will need to use a wrapper to maintain a separate
    /// contiguous buffer as required.
    ///
    /// In practice, these types should be using a [`TextLayout`] object
    /// per paragraph, and in general a separate buffer will be unnecessary.
    ///
    /// [`TextLayout`]: trait.TextLayout.html
    fn as_str(&self) -> &str;
}

impl std::ops::Deref for dyn TextStorage {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

/// Attributes that can be applied to text.
pub enum TextAttribute {
    /// The font family.
    FontFamily(FontFamily),
    /// The font size, in points.
    FontSize(f64),
    /// The [`FontWeight`](struct.FontWeight.html).
    Weight(FontWeight),
    /// The foreground color of the text.
    TextColor(crate::Color),
    /// The [`FontStyle`]; either regular or italic.
    ///
    /// [`FontStyle`]: enum.FontStyle.html
    Style(FontStyle),
    /// Underline.
    Underline(bool),
    /// Strikethrough.
    Strikethrough(bool),
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
    ///     .default_attribute(TextAttribute::FontFamily(times.clone()))
    ///     .default_attribute(TextAttribute::FontSize(12.0))
    ///     .build();
    /// ```
    fn font(self, font: FontFamily, font_size: f64) -> Self {
        self.default_attribute(TextAttribute::FontFamily(font))
            .default_attribute(TextAttribute::FontSize(font_size))
    }

    /// A convenience method for setting the default text color.
    ///
    /// This is equivalent to passing `TextAttribute::TextColor` to the
    /// `default_attribute` method.
    fn text_color(self, color: Color) -> Self {
        self.default_attribute(TextAttribute::TextColor(color))
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
    ///     .default_attribute(FontStyle::Italic)
    ///     .range_attribute(..5, FontWeight::BOLD)
    ///     .range_attribute(5..14, times)
    ///     .range_attribute(20.., TextAttribute::TextColor(Color::rgb(1.0, 0., 0.,)))
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
/// A text layout may be broken into multiple lines in order to fit within a given width.
/// Line breaking is generally done between words (whitespace-separated).
///
/// A line's text and [`LineMetric`][]s can be accessed by 0-indexed line number.
///
/// ## Text Position
///
/// A text position is the offset in the underlying string, defined in utf-8
/// code units, as is standard for Rust strings.
///
/// However, text position is also related to valid cursor positions. Therefore:
/// - The beginning of a line has text position `0`.
/// - The end of a line is a valid text position. e.g. `text.len()` is a valid text position.
/// - If the text position is not at a code point or grapheme boundary, undesirable behavior may
/// occur.
///
/// [`LineMetric`]: struct.LineMetric.html
pub trait TextLayout: Clone {
    /// The total size of this `TextLayout`.
    ///
    /// This is the size required to draw this `TextLayout`, as provided by the
    /// platform text system.
    ///
    /// If the layout is empty (the text is the empty string) the returned
    /// `Size` will have the height required to draw a cursor in the layout's
    /// default font.
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

    /// The width of this layout, including the width of any trailing whitespace.
    ///
    /// In many situations you do not want to include the width of trailing
    /// whitespace when measuring width; for instance when word-wrap is enabled,
    /// trailing whitespace is ignored. In other circumstances, however, this
    /// width is important, such as when editing a single line of text; in these
    /// cases you want to use this method to ensure you account for the actual
    /// size of any trailing whitespace.
    fn trailing_whitespace_width(&self) -> f64;

    /// Returns a `Rect` representing the bounding box of the glyphs in this layout,
    /// relative to the top-left of the layout object.
    ///
    /// This is sometimes called the bounding box or the inking rect, and is
    /// used to determine when the layout has become visible (for instance,
    /// during scrolling) and thus needs to be drawn.
    fn image_bounds(&self) -> Rect;

    /// The text used to create this layout.
    fn text(&self) -> &str;

    /// Given a line number, return a reference to that line's underlying string.
    ///
    /// This will include any trailing whitespace.
    fn line_text(&self, line_number: usize) -> Option<&str>;

    /// Given a line number, return a reference to that line's metrics, if the line exists.
    ///
    /// If this layout's text is the empty string, calling this method with `0`
    /// returns some [`LineMetric`]; this will use the layout's default font to
    /// determine what the expected height of the first line would be, which is
    /// necessary for things like cursor drawing.
    ///
    /// [`LineMetric`]: struct.LineMetric.html
    fn line_metric(&self, line_number: usize) -> Option<LineMetric>;

    /// Returns total number of lines in the text layout.
    ///
    /// The return value will always be greater than 0; a layout of the empty
    /// string is considered to have a single line.
    fn line_count(&self) -> usize;

    /// Given a `Point`, return a [`HitTestPoint`] describing the corresponding
    /// text position.
    ///
    /// This is used for things like mapping a mouse click to a cursor position.
    ///
    /// The point should be in the coordinate space of the layout object.
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
    /// return a [`HitTestPosition`] object describing the location of that boundary
    /// within the layout.
    ///
    /// For more on text positions, see docs for the [`TextLayout`] trait.
    ///
    /// ## Notes:
    ///
    /// The user is expected to ensure that the provided index is a grapheme
    /// boundary. If it is a character boundary but *not* a grapheme boundary,
    /// the return value may be backend-specific.
    ///
    /// ## Panics:
    ///
    /// This method will panic if the text position is not a character boundary,
    ///
    /// [`HitTestPosition`]: struct.HitTestPosition.html
    /// [`TextLayout`]: ../piet/trait.TextLayout.html
    fn hit_test_text_position(&self, idx: usize) -> HitTestPosition;

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

        if range.start >= range.end {
            return Vec::new();
        }

        let first_line = self.hit_test_text_position(range.start).line;
        let last_line = self.hit_test_text_position(range.end).line;

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

            let start_x = self.hit_test_text_position(line_range_start).point.x;
            //HACK: because we don't have affinity, if the line has an emergency
            //break we need to manually use the layout width as the end point
            //for the selection rect. See https://github.com/linebender/piet/issues/323
            let end_x = if line != last_line && metrics.trailing_whitespace == 0 {
                self.size().width
            } else {
                self.hit_test_text_position(line_range_end).point.x
            };
            result.push(Rect::new(start_x, y0, end_x, y1));
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

    /// The length of the trailing whitespace at the end of this line, in utf-8
    /// code units.
    ///
    /// When lines are broken on whitespace (as is common), the whitespace
    /// is assigned to the end of the preceding line. Reporting the size of
    /// the trailing whitespace section lets an API consumer measure and render
    /// only the trimmed line up to the whitespace.
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
        TextAttribute::FontFamily(t)
    }
}

impl From<FontWeight> for TextAttribute {
    fn from(src: FontWeight) -> TextAttribute {
        TextAttribute::Weight(src)
    }
}

impl From<FontStyle> for TextAttribute {
    fn from(src: FontStyle) -> TextAttribute {
        TextAttribute::Style(src)
    }
}

impl Default for TextAlignment {
    fn default() -> Self {
        TextAlignment::Start
    }
}

impl TextStorage for std::sync::Arc<str> {
    fn as_str(&self) -> &str {
        self
    }
}

impl TextStorage for std::rc::Rc<str> {
    fn as_str(&self) -> &str {
        self
    }
}

impl TextStorage for String {
    fn as_str(&self) -> &str {
        self.as_str()
    }
}

impl TextStorage for std::sync::Arc<String> {
    fn as_str(&self) -> &str {
        self
    }
}

impl TextStorage for std::rc::Rc<String> {
    fn as_str(&self) -> &str {
        self
    }
}

impl TextStorage for &'static str {
    fn as_str(&self) -> &str {
        self
    }
}
