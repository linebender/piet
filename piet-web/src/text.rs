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

use self::grapheme::point_x_in_grapheme;
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

    // first assume one line.
    // TODO do with lines
    fn hit_test_point(&self, point: Point) -> HitTestPoint {
        // internal logic is using grapheme clusters, but return the text position associated
        // with the border of the grapheme cluster.

        // null case
        if self.text.is_empty() {
            return HitTestPoint::default();
        }

        // get bounds
        // TODO handle if string is not null yet count is 0?
        let end = UnicodeSegmentation::graphemes(self.text.as_str(), true).count() - 1;
        let end_bounds = match self.get_grapheme_boundaries(end) {
            Some(bounds) => bounds,
            None => return HitTestPoint::default(),
        };

        let start = 0;
        let start_bounds = match self.get_grapheme_boundaries(start) {
            Some(bounds) => bounds,
            None => return HitTestPoint::default(),
        };

        // first test beyond ends
        if point.x > end_bounds.trailing {
            let mut res = HitTestPoint::default();
            res.metrics.text_position = self.text.len();
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

            let grapheme_bounds = match self.get_grapheme_boundaries(middle) {
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

    fn hit_test_text_position(&self, text_position: usize) -> Option<HitTestTextPosition> {
        // Using substrings, but now with unicode grapheme awareness

        let text_len = self.text.len();

        if text_position == 0 {
            return Some(HitTestTextPosition::default());
        }

        if text_position as usize >= text_len {
            let x = self.width();

            return Some(HitTestTextPosition {
                point: Point { x, y: 0.0 },
                metrics: HitTestMetrics {
                    text_position: text_len,
                },
            });
        }

        // Already checked that text_position > 0 and text_position < count.
        // If text position is not at a grapheme boundary, use the text position of current
        // grapheme cluster. But return the original text position
        // Use the indices (byte offset, which for our purposes = utf8 code units).
        let grapheme_indices = UnicodeSegmentation::grapheme_indices(self.text.as_str(), true)
            .take_while(|(byte_idx, _s)| text_position >= *byte_idx);

        if let Some((byte_idx, _s)) = grapheme_indices.last() {
            let x = self
                .ctx
                .measure_text(&self.text[0..byte_idx])
                .map(|m| m.width())
                .expect("Text measurement failed");

            Some(HitTestTextPosition {
                point: Point { x, y: 0.0 },
                metrics: HitTestMetrics { text_position },
            })
        } else {
            // iterated to end boundary
            Some(HitTestTextPosition {
                point: Point {
                    x: self.width(),
                    y: 0.0,
                },
                metrics: HitTestMetrics {
                    text_position: text_len,
                },
            })
        }
    }
}

pub(crate) fn text_width(text: &str, ctx: &CanvasRenderingContext2d) -> f64 {
    ctx.measure_text(text)
        .map(|m| m.width())
        .expect("Text measurement failed")
}
