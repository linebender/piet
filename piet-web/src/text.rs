//! Text functionality for Piet web backend

mod grapheme;
mod lines;

use std::borrow::Cow;

use web_sys::CanvasRenderingContext2d;

use piet::kurbo::Point;

use piet::{
    Error, Font, FontBuilder, HitTestMetrics, HitTestPoint, HitTestTextPosition, LineMetric, Text,
    TextLayout, TextLayoutBuilder,
};
use unicode_segmentation::UnicodeSegmentation;

use self::grapheme::{get_grapheme_boundaries, point_x_in_grapheme};
use crate::WebRenderContext;

#[derive(Clone)]
pub struct WebFont {
    family: String,
    weight: u32,
    style: FontStyle,
    size: f64,
}

pub struct WebFontBuilder(WebFont);

#[derive(Clone)]
pub struct WebTextLayout {
    ctx: CanvasRenderingContext2d,
    // TODO like cairo, should this be pub(crate)?
    pub font: WebFont,
    pub text: String,

    // Calculated on build
    line_metrics: Vec<LineMetric>,
    width: f64,
}

pub struct WebTextLayoutBuilder {
    ctx: CanvasRenderingContext2d,
    font: WebFont,
    text: String,
    width: f64,
}

/// https://developer.mozilla.org/en-US/docs/Web/CSS/font-style
#[allow(dead_code)] // TODO: Remove
#[derive(Clone)]
enum FontStyle {
    Normal,
    Italic,
    Oblique(Option<f64>),
}

impl<'a> Text for WebRenderContext<'a> {
    type Font = WebFont;
    type FontBuilder = WebFontBuilder;
    type TextLayout = WebTextLayout;
    type TextLayoutBuilder = WebTextLayoutBuilder;

    fn new_font_by_name(&mut self, name: &str, size: f64) -> Self::FontBuilder {
        let font = WebFont {
            family: name.to_owned(),
            size,
            weight: 400,
            style: FontStyle::Normal,
        };
        WebFontBuilder(font)
    }

    fn new_text_layout(
        &mut self,
        font: &Self::Font,
        text: &str,
        width: f64,
    ) -> Self::TextLayoutBuilder {
        WebTextLayoutBuilder {
            // TODO: it's very likely possible to do this without cloning ctx, but
            // I couldn't figure out the lifetime errors from a `&'a` reference.
            ctx: self.ctx.clone(),
            font: font.clone(),
            text: text.to_owned(),
            width,
        }
    }
}

impl FontBuilder for WebFontBuilder {
    type Out = WebFont;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(self.0)
    }
}

impl Font for WebFont {}

impl WebFont {
    // TODO should this be pub(crate)?
    pub fn get_font_string(&self) -> String {
        let style_str = match self.style {
            FontStyle::Normal => Cow::from("normal"),
            FontStyle::Italic => Cow::from("italic"),
            FontStyle::Oblique(None) => Cow::from("italic"),
            FontStyle::Oblique(Some(angle)) => Cow::from(format!("oblique {}deg", angle)),
        };
        format!(
            "{} {} {}px \"{}\"",
            style_str, self.weight, self.size, self.family
        )
    }
}

impl TextLayoutBuilder for WebTextLayoutBuilder {
    type Out = WebTextLayout;

    fn build(self) -> Result<Self::Out, Error> {
        self.ctx.set_font(&self.font.get_font_string());

        let line_metrics =
            lines::calculate_line_metrics(&self.text, &self.ctx, self.width, self.font.size);

        let widths = line_metrics.iter().map(|lm| {
            text_width(
                &self.text[lm.start_offset..lm.end_offset - lm.trailing_whitespace],
                &self.ctx,
            )
        });

        // TODO default width 0?
        let width = widths.fold(0.0, |a: f64, b| a.max(b));

        Ok(WebTextLayout {
            ctx: self.ctx,
            font: self.font,
            text: self.text,
            line_metrics,
            width,
        })
    }
}

impl TextLayout for WebTextLayout {
    fn width(&self) -> f64 {
        // precalculated on textlayout build
        self.width
    }

    fn update_width(&mut self, new_width: f64) -> Result<(), Error> {
        self.width = new_width;
        self.line_metrics =
            lines::calculate_line_metrics(&self.text, &self.ctx, new_width, self.font.size);
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
        // internal logic is using grapheme clusters, but return the text position associated
        // with the border of the grapheme cluster.

        // null case
        if self.text.is_empty() {
            return HitTestPoint::default();
        }

        // this assumes that all heights/baselines are the same.
        // Uses line bounding box to do hit testpoint, but with coordinates starting at 0.0 at
        // first baseline
        let first_baseline = self.line_metrics.get(0).map(|l| l.baseline).unwrap_or(0.0);

        // check out of bounds above top
        if point.y < -1.0 * first_baseline {
            return HitTestPoint::default();
        }

        // get the line metric
        let mut lm = self
            .line_metrics
            .iter()
            .skip_while(|l| l.cumulative_height - first_baseline < point.y);
        let lm = match lm.next() {
            Some(lm) => lm,
            None => {
                // this means it went over on y axis, so it returns last text position
                let mut htp = HitTestPoint::default();
                htp.metrics.text_position = self.text.len();
                return htp;
            }
        };

        // Then for the line, do hit test point
        // Trailing whitespace is remove for the line
        let line = &self.text[lm.start_offset..lm.end_offset - lm.trailing_whitespace];

        let mut htp = hit_test_line_point(&self.ctx, line, &point);
        htp.metrics.text_position += lm.start_offset;
        htp
    }

    fn hit_test_text_position(&self, text_position: usize) -> Option<HitTestTextPosition> {
        // first need to find line it's on, and get line start offset
        let lm = self
            .line_metrics
            .iter()
            .take_while(|l| l.start_offset <= text_position)
            .last()
            .cloned()
            .unwrap_or_else(Default::default);

        let count = self
            .line_metrics
            .iter()
            .take_while(|l| l.start_offset <= text_position)
            .count();

        // In web toy text, all baselines and heights are the same.
        // We're counting the first line baseline as 0, and measuring to each line's baseline.
        let y = if count == 0 {
            return Some(HitTestTextPosition::default());
        } else {
            (count - 1) as f64 * lm.height
        };

        // Then for the line, do text position
        // Trailing whitespace is removed for the line
        let line = &self.text[lm.start_offset..lm.end_offset - lm.trailing_whitespace];
        let line_position = text_position - lm.start_offset;

        let mut http = hit_test_line_position(&self.ctx, line, line_position);
        if let Some(h) = http.as_mut() {
            h.point.y = y;
            h.metrics.text_position += lm.start_offset;
        };
        http
    }
}

// NOTE this is the same as the old, non-line-aware version of hit_test_point
// Future: instead of passing ctx, should there be some other line-level text layout?
fn hit_test_line_point(ctx: &CanvasRenderingContext2d, text: &str, point: &Point) -> HitTestPoint {
    // null case
    if text.is_empty() {
        return HitTestPoint::default();
    }

    // get bounds
    // TODO handle if string is not null yet count is 0?
    let end = UnicodeSegmentation::graphemes(text, true).count() - 1;
    let end_bounds = match get_grapheme_boundaries(ctx, text, end) {
        Some(bounds) => bounds,
        None => return HitTestPoint::default(),
    };

    let start = 0;
    let start_bounds = match get_grapheme_boundaries(ctx, text, start) {
        Some(bounds) => bounds,
        None => return HitTestPoint::default(),
    };

    // first test beyond ends
    if point.x > end_bounds.trailing {
        let mut res = HitTestPoint::default();
        res.metrics.text_position = text.len();
        return res;
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

        let grapheme_bounds = match get_grapheme_boundaries(ctx, text, middle) {
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
// Future: instead of passing ctx, should there be some other line-level text layout?
fn hit_test_line_position(
    ctx: &CanvasRenderingContext2d,
    text: &str,
    text_position: usize,
) -> Option<HitTestTextPosition> {
    // Using substrings with unicode grapheme awareness

    let text_len = text.len();

    if text_position == 0 {
        return Some(HitTestTextPosition::default());
    }

    if text_position as usize >= text_len {
        return Some(HitTestTextPosition {
            point: Point {
                x: text_width(text, ctx),
                y: 0.0,
            },
            metrics: HitTestMetrics {
                text_position: text_len,
            },
        });
    }

    // Already checked that text_position > 0 and text_position < count.
    // If text position is not at a grapheme boundary, use the text position of current
    // grapheme cluster. But return the original text position
    // Use the indices (byte offset, which for our purposes = utf8 code units).
    let grapheme_indices = UnicodeSegmentation::grapheme_indices(text, true)
        .take_while(|(byte_idx, _s)| text_position >= *byte_idx);

    if let Some((byte_idx, _s)) = grapheme_indices.last() {
        let point_x = text_width(&text[0..byte_idx], ctx);

        Some(HitTestTextPosition {
            point: Point { x: point_x, y: 0.0 },
            metrics: HitTestMetrics { text_position },
        })
    } else {
        // iterated to end boundary
        Some(HitTestTextPosition {
            point: Point {
                x: text_width(text, ctx),
                y: 0.0,
            },
            metrics: HitTestMetrics {
                text_position: text_len,
            },
        })
    }
}

pub(crate) fn text_width(text: &str, ctx: &CanvasRenderingContext2d) -> f64 {
    ctx.measure_text(text)
        .map(|m| m.width())
        .expect("Text measurement failed")
}
