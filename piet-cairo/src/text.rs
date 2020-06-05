//! Text functionality for Piet cairo backend

mod grapheme;
mod lines;

use std::marker::PhantomData;

use cairo::{FontFace, FontOptions, FontSlant, FontWeight, Matrix, ScaledFont};

use piet::kurbo::Point;

use piet::{
    Error, Font, FontBuilder, HitTestMetrics, HitTestPoint, HitTestTextPosition, LineMetric,
    RoundInto, Text, TextLayout, TextLayoutBuilder,
};

use unicode_segmentation::UnicodeSegmentation;

use self::grapheme::{get_grapheme_boundaries, point_x_in_grapheme};

/// Right now, we don't need any state, as the "toy text API" treats the
/// access to system font information as a global. This will change.
// we use a phantom lifetime here to match the API of the d2d backend,
// and the likely API of something with access to system font information.
#[derive(Clone)]
pub struct CairoText<'a>(PhantomData<&'a ()>);

pub struct CairoFont(ScaledFont);

pub struct CairoFontBuilder {
    family: String,
    weight: FontWeight,
    slant: FontSlant,
    size: f64,
}

#[derive(Clone)]
pub struct CairoTextLayout {
    // TODO should these fields be pub(crate)?
    width: f64,
    pub font: ScaledFont,
    pub text: String,

    // currently calculated on build
    pub(crate) line_metrics: Vec<LineMetric>,
}

pub struct CairoTextLayoutBuilder(CairoTextLayout);

impl<'a> CairoText<'a> {
    /// Create a new factory that satisfies the piet `Text` trait.
    ///
    /// No state is needed for now because the current implementation is just
    /// toy text, but that will change when proper text is implemented.
    #[allow(clippy::new_without_default)]
    pub fn new() -> CairoText<'a> {
        CairoText(PhantomData)
    }
}

impl<'a> Text for CairoText<'a> {
    type Font = CairoFont;
    type FontBuilder = CairoFontBuilder;
    type TextLayout = CairoTextLayout;
    type TextLayoutBuilder = CairoTextLayoutBuilder;

    fn new_font_by_name(&mut self, name: &str, size: f64) -> Self::FontBuilder {
        CairoFontBuilder {
            family: name.to_owned(),
            size: size.round_into(),
            weight: FontWeight::Normal,
            slant: FontSlant::Normal,
        }
    }

    fn new_text_layout(
        &mut self,
        font: &Self::Font,
        text: &str,
        width: impl Into<Option<f64>>,
    ) -> Self::TextLayoutBuilder {
        // TODO Should probably move the calculations to `build` step
        let width = width.into().unwrap_or(std::f64::INFINITY);

        let line_metrics = lines::calculate_line_metrics(text, &font.0, width);

        let widths = line_metrics.iter().map(|lm| {
            font.0
                .text_extents(&text[lm.start_offset..lm.end_offset])
                .x_advance
        });

        let width = widths.fold(0.0, |a: f64, b| a.max(b));

        let text_layout = CairoTextLayout {
            width,
            font: font.0.clone(),
            text: text.to_owned(),
            line_metrics,
        };
        CairoTextLayoutBuilder(text_layout)
    }
}

impl FontBuilder for CairoFontBuilder {
    type Out = CairoFont;

    fn build(self) -> Result<Self::Out, Error> {
        let font_face = FontFace::toy_create(&self.family, self.slant, self.weight);
        let font_matrix = scale_matrix(self.size);
        let ctm = scale_matrix(1.0);
        let options = FontOptions::default();
        let scaled_font = ScaledFont::new(&font_face, &font_matrix, &ctm, &options);
        Ok(CairoFont(scaled_font))
    }
}

impl Font for CairoFont {}

impl TextLayoutBuilder for CairoTextLayoutBuilder {
    type Out = CairoTextLayout;

    fn build(self) -> Result<Self::Out, Error> {
        Ok(self.0)
    }
}

impl TextLayout for CairoTextLayout {
    fn width(&self) -> f64 {
        // calculated by max x_advance, on TextLayout build
        self.width
    }

    // TODO refactor this to use same code as new_text_layout
    fn update_width(&mut self, new_width: impl Into<Option<f64>>) -> Result<(), Error> {
        let new_width = new_width.into().unwrap_or(std::f64::INFINITY);

        self.line_metrics = lines::calculate_line_metrics(&self.text, &self.font, new_width);

        let widths = self.line_metrics.iter().map(|lm| {
            self.font
                .text_extents(&self.text[lm.start_offset..lm.end_offset])
                .x_advance
        });

        self.width = widths.fold(0.0, |a: f64, b| a.max(b));

        Ok(())
    }

    fn line_text(&self, line_number: usize) -> Option<&str> {
        self.line_metrics
            .get(line_number)
            .map(|lm| &self.text[lm.start_offset..(lm.end_offset)])
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
        // out of bounds on bottom during iteration
        let mut is_y_inside = true;
        if point.y < -1.0 * first_baseline {
            is_y_inside = false
        };

        // get the line metric
        let mut lm = self
            .line_metrics
            .iter()
            .skip_while(|l| l.cumulative_height - first_baseline < point.y);
        let lm = lm
            .next()
            .or_else(|| {
                // This means it went over the last line, so return the last line.
                is_y_inside = false;
                self.line_metrics.last()
            })
            .cloned() // TODO remove this clone?
            .unwrap_or_else(|| {
                is_y_inside = false;
                Default::default()
            });

        // Then for the line, do hit test point
        // Trailing whitespace is remove for the line
        let line = &self.text[lm.start_offset..lm.end_offset];

        let mut htp = hit_test_line_point(&self.font, line, &point);
        htp.metrics.text_position += lm.start_offset;

        if !is_y_inside {
            htp.is_inside = false;
        }

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

        // In cairo toy text, all baselines and heights are the same.
        // We're counting the first line baseline as 0, and measuring to each line's baseline.
        let y = if count == 0 {
            return Some(HitTestTextPosition::default());
        } else {
            (count - 1) as f64 * lm.height
        };

        // Then for the line, do text position
        // Trailing whitespace is removed for the line
        let line = &self.text[lm.start_offset..lm.end_offset];
        let line_position = text_position - lm.start_offset;

        let mut http = hit_test_line_position(&self.font, line, line_position);
        if let Some(h) = http.as_mut() {
            h.point.y = y;
            h.metrics.text_position += lm.start_offset;
        };
        http
    }
}

// NOTE this is the same as the old, non-line-aware version of hit_test_point
// Future: instead of passing Font, should there be some other line-level text layout?
fn hit_test_line_point(font: &ScaledFont, text: &str, point: &Point) -> HitTestPoint {
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
fn hit_test_line_position(
    font: &ScaledFont,
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
                x: font.text_extents(&text).x_advance,
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
        let point_x = font.text_extents(&text[0..byte_idx]).x_advance;

        Some(HitTestTextPosition {
            point: Point { x: point_x, y: 0.0 },
            metrics: HitTestMetrics { text_position },
        })
    } else {
        // iterated to end boundary
        Some(HitTestTextPosition {
            point: Point {
                x: font.text_extents(&text).x_advance,
                y: 0.0,
            },
            metrics: HitTestMetrics {
                text_position: text_len,
            },
        })
    }
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

    // - x: calculated value
    // - target: f64
    // - tolerance: in f64
    fn assert_close_to(x: f64, target: f64, tolerance: f64) {
        let min = target - tolerance;
        let max = target + tolerance;
        println!("x: {}, target: {}", x, target);
        assert!(x <= max && x >= min);
    }

    #[test]
    fn test_hit_test_text_position_basic() {
        let mut text_layout = CairoText::new();

        let input = "piet text!";
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();

        let layout = text_layout
            .new_text_layout(&font, &input[0..4], std::f64::INFINITY)
            .build()
            .unwrap();
        let piet_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..3], std::f64::INFINITY)
            .build()
            .unwrap();
        let pie_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..2], std::f64::INFINITY)
            .build()
            .unwrap();
        let pi_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..1], std::f64::INFINITY)
            .build()
            .unwrap();
        let p_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, "", std::f64::INFINITY)
            .build()
            .unwrap();
        let null_width = layout.width();

        let full_layout = text_layout
            .new_text_layout(&font, input, std::f64::INFINITY)
            .build()
            .unwrap();
        let full_width = full_layout.width();

        assert_close_to(
            full_layout.hit_test_text_position(4).unwrap().point.x as f64,
            piet_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(3).unwrap().point.x as f64,
            pie_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(2).unwrap().point.x as f64,
            pi_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(1).unwrap().point.x as f64,
            p_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(0).unwrap().point.x as f64,
            null_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(10).unwrap().point.x as f64,
            full_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(11).unwrap().point.x as f64,
            full_width,
            3.0,
        );
        assert_eq!(
            full_layout
                .hit_test_text_position(11)
                .unwrap()
                .metrics
                .text_position,
            10
        )
    }

    #[test]
    fn test_hit_test_text_position_complex_0() {
        let input = "é";
        assert_eq!(input.len(), 2);

        let mut text_layout = CairoText::new();
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, input, std::f64::INFINITY)
            .build()
            .unwrap();

        assert_close_to(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close_to(
            layout.hit_test_text_position(2).unwrap().point.x,
            layout.width(),
            3.0,
        );

        // note code unit not at grapheme boundary
        // This one panics in d2d because this is not a code unit boundary.
        // But it works here! Harder to deal with this right now, since unicode-segmentation
        // doesn't give code point offsets.
        assert_close_to(layout.hit_test_text_position(1).unwrap().point.x, 0.0, 3.0);
        assert_eq!(
            layout
                .hit_test_text_position(1)
                .unwrap()
                .metrics
                .text_position,
            1
        );

        // unicode segmentation is wrong on this one for now.
        //let input = "🤦\u{1f3fc}\u{200d}\u{2642}\u{fe0f}";

        //let mut text_layout = D2DText::new();
        //let font = text_layout.new_font_by_name("sans-serif", 12.0).build().unwrap();
        //let layout = text_layout.new_text_layout(&font, input, std::f64::INFINITY).build().unwrap();

        //assert_eq!(input.graphemes(true).count(), 1);
        //assert_eq!(layout.hit_test_text_position(0, true).map(|p| p.point_x as f64), Some(layout.width()));
        //assert_eq!(input.len(), 17);

        let input = "\u{0023}\u{FE0F}\u{20E3}"; // #️⃣
        assert_eq!(input.len(), 7);
        assert_eq!(input.chars().count(), 3);

        let mut text_layout = CairoText::new();
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, input, std::f64::INFINITY)
            .build()
            .unwrap();

        assert_close_to(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close_to(
            layout.hit_test_text_position(7).unwrap().point.x,
            layout.width(),
            3.0,
        );

        // note code unit not at grapheme boundary
        assert_close_to(layout.hit_test_text_position(1).unwrap().point.x, 0.0, 3.0);
        assert_eq!(
            layout
                .hit_test_text_position(1)
                .unwrap()
                .metrics
                .text_position,
            1
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
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, input, std::f64::INFINITY)
            .build()
            .unwrap();

        let test_layout_0 = text_layout
            .new_text_layout(&font, &input[0..2], std::f64::INFINITY)
            .build()
            .unwrap();
        let test_layout_1 = text_layout
            .new_text_layout(&font, &input[0..9], std::f64::INFINITY)
            .build()
            .unwrap();
        let test_layout_2 = text_layout
            .new_text_layout(&font, &input[0..10], std::f64::INFINITY)
            .build()
            .unwrap();

        // Note: text position is in terms of utf8 code units
        assert_close_to(layout.hit_test_text_position(0).unwrap().point.x, 0.0, 3.0);
        assert_close_to(
            layout.hit_test_text_position(2).unwrap().point.x,
            test_layout_0.width(),
            3.0,
        );
        assert_close_to(
            layout.hit_test_text_position(9).unwrap().point.x,
            test_layout_1.width(),
            3.0,
        );
        assert_close_to(
            layout.hit_test_text_position(10).unwrap().point.x,
            test_layout_2.width(),
            3.0,
        );
        assert_close_to(
            layout.hit_test_text_position(14).unwrap().point.x,
            layout.width(),
            3.0,
        );

        // Code point boundaries, but not grapheme boundaries.
        // Width should stay at the current grapheme boundary.
        assert_close_to(
            layout.hit_test_text_position(3).unwrap().point.x,
            test_layout_0.width(),
            3.0,
        );
        assert_eq!(
            layout
                .hit_test_text_position(3)
                .unwrap()
                .metrics
                .text_position,
            3
        );
        assert_close_to(
            layout.hit_test_text_position(6).unwrap().point.x,
            test_layout_0.width(),
            3.0,
        );
        assert_eq!(
            layout
                .hit_test_text_position(6)
                .unwrap()
                .metrics
                .text_position,
            6
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_hit_test_point_basic_0() {
        let mut text_layout = CairoText::new();

        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, "piet text!", std::f64::INFINITY)
            .build()
            .unwrap();
        println!("text pos 4: {:?}", layout.hit_test_text_position(4)); // 23.0
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 27.0

        // test hit test point
        // all inside
        let pt = layout.hit_test_point(Point::new(22.5, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(25.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(26.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(27.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(28.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);

        // outside
        println!("layout_width: {:?}", layout.width()); // 56.0

        let pt = layout.hit_test_point(Point::new(56.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10); // last text position
        assert_eq!(pt.is_inside, true);

        let pt = layout.hit_test_point(Point::new(57.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10); // last text position
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(-1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0); // first text position
        assert_eq!(pt.is_inside, false);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_hit_test_point_basic_0() {
        let mut text_layout = CairoText::new();

        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, "piet text!", std::f64::INFINITY)
            .build()
            .unwrap();
        println!("text pos 4: {:?}", layout.hit_test_text_position(4)); // 19.34765625
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 22.681640625

        // test hit test point
        // all inside
        let pt = layout.hit_test_point(Point::new(19.0, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(20.0, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(21.0, 0.0));
        assert_eq!(pt.metrics.text_position, 4);
        let pt = layout.hit_test_point(Point::new(22.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.metrics.text_position, 5);

        // outside
        println!("layout_width: {:?}", layout.width()); //45.357421875

        let pt = layout.hit_test_point(Point::new(45.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10); // last text position
        assert_eq!(pt.is_inside, true);

        let pt = layout.hit_test_point(Point::new(46.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10); // last text position
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(-1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0); // first text position
        assert_eq!(pt.is_inside, false);
    }

    #[test]
    #[cfg(target_os = "linux")]
    // for testing that 'middle' assignment in binary search is correct
    fn test_hit_test_point_basic_1() {
        let mut text_layout = CairoText::new();

        // base condition, one grapheme
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, "t", std::f64::INFINITY)
            .build()
            .unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0

        // two graphemes (to check that middle moves)
        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);

        let layout = text_layout
            .new_text_layout(&font, "te", std::f64::INFINITY)
            .build()
            .unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 12.0

        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.metrics.text_position, 1);
        let pt = layout.hit_test_point(Point::new(6.0, 0.0));
        assert_eq!(pt.metrics.text_position, 1);
        let pt = layout.hit_test_point(Point::new(11.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
    }

    #[test]
    #[cfg(target_os = "macos")]
    // for testing that 'middle' assignment in binary search is correct
    fn test_hit_test_point_basic_1() {
        let mut text_layout = CairoText::new();

        // base condition, one grapheme
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, "t", std::f64::INFINITY)
            .build()
            .unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0

        // two graphemes (to check that middle moves)
        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);

        let layout = text_layout
            .new_text_layout(&font, "te", std::f64::INFINITY)
            .build()
            .unwrap();
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 12.0

        let pt = layout.hit_test_point(Point::new(1.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.metrics.text_position, 1);
        let pt = layout.hit_test_point(Point::new(6.0, 0.0));
        assert_eq!(pt.metrics.text_position, 1);
        let pt = layout.hit_test_point(Point::new(11.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
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
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, input, std::f64::INFINITY)
            .build()
            .unwrap();
        //println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 6.99999999
        //println!("text pos 9: {:?}", layout.hit_test_text_position(9)); // 24.0
        //println!("text pos 10: {:?}", layout.hit_test_text_position(10)); // 32.0
        //println!("text pos 14: {:?}", layout.hit_test_text_position(14)); // 39.0, line width

        let pt = layout.hit_test_point(Point::new(2.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(7.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(10.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(14.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(18.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(26.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(29.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(32.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(35.5, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
        let pt = layout.hit_test_point(Point::new(38.0, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
        let pt = layout.hit_test_point(Point::new(40.0, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
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
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, input, std::f64::INFINITY)
            .build()
            .unwrap();
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 6.673828125
        println!("text pos 9: {:?}", layout.hit_test_text_position(9)); // 28.55859375
        println!("text pos 10: {:?}", layout.hit_test_text_position(10)); // 35.232421875
        println!("text pos 14: {:?}", layout.hit_test_text_position(14)); // 42.8378905, line width

        let pt = layout.hit_test_point(Point::new(2.0, 0.0));
        assert_eq!(pt.metrics.text_position, 0);
        let pt = layout.hit_test_point(Point::new(4.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(7.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(10.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(14.0, 0.0));
        assert_eq!(pt.metrics.text_position, 2);
        let pt = layout.hit_test_point(Point::new(18.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(23.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(26.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(29.0, 0.0));
        assert_eq!(pt.metrics.text_position, 9);
        let pt = layout.hit_test_point(Point::new(32.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(35.5, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(38.0, 0.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(40.0, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
        let pt = layout.hit_test_point(Point::new(43.0, 0.0));
        assert_eq!(pt.metrics.text_position, 14);
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
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, input, std::f64::INFINITY)
            .build()
            .unwrap();
        println!("text pos 0: {:?}", layout.hit_test_text_position(0)); // 0.0
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 5.0
        println!("text pos 3: {:?}", layout.hit_test_text_position(3)); // 13.0
        println!("text pos 4: {:?}", layout.hit_test_text_position(4)); // 13.0
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 21.0
        println!("text pos 6: {:?}", layout.hit_test_text_position(6)); // 28.0
        println!("text pos 7: {:?}", layout.hit_test_text_position(7)); // 36.0
        println!("text pos 8: {:?}", layout.hit_test_text_position(8)); // 39.0, end

        let pt = layout.hit_test_point(Point::new(27.0, 0.0));
        assert_eq!(pt.metrics.text_position, 6);
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
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();
        let layout = text_layout
            .new_text_layout(&font, input, std::f64::INFINITY)
            .build()
            .unwrap();
        println!("text pos 0: {:?}", layout.hit_test_text_position(0)); // 0.0
        println!("text pos 1: {:?}", layout.hit_test_text_position(1)); // 5.0
        println!("text pos 2: {:?}", layout.hit_test_text_position(2)); // 5.0
        println!("text pos 3: {:?}", layout.hit_test_text_position(3)); // 13.0
        println!("text pos 4: {:?}", layout.hit_test_text_position(4)); // 13.0
        println!("text pos 5: {:?}", layout.hit_test_text_position(5)); // 21.0
        println!("text pos 6: {:?}", layout.hit_test_text_position(6)); // 28.0
        println!("text pos 7: {:?}", layout.hit_test_text_position(7)); // 36.0
        println!("text pos 8: {:?}", layout.hit_test_text_position(8)); // 39.0, end

        let pt = layout.hit_test_point(Point::new(27.0, 0.0));
        assert_eq!(pt.metrics.text_position, 6);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_multiline_hit_test_text_position_basic() {
        let mut text_layout = CairoText::new();

        let input = "piet  text!";
        let font = text_layout
            .new_font_by_name("sans-serif", 12.0)
            .build()
            .unwrap();

        let layout = text_layout
            .new_text_layout(&font, &input[0..3], 30.0)
            .build()
            .unwrap();
        let pie_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..4], 25.0)
            .build()
            .unwrap();
        let piet_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..5], 30.0)
            .build()
            .unwrap();
        let piet_space_width = layout.width();

        // "text" should be on second line
        let layout = text_layout
            .new_text_layout(&font, &input[6..10], 25.0)
            .build()
            .unwrap();
        let text_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[6..9], 25.0)
            .build()
            .unwrap();
        let tex_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[6..8], 25.0)
            .build()
            .unwrap();
        let te_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[6..7], 25.0)
            .build()
            .unwrap();
        let t_width = layout.width();

        let full_layout = text_layout
            .new_text_layout(&font, input, 25.0)
            .build()
            .unwrap();

        println!("lm: {:#?}", full_layout.line_metrics);
        println!("layout width: {:#?}", full_layout.width());

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
        assert_close_to(
            full_layout.hit_test_text_position(10).unwrap().point.x as f64,
            text_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(9).unwrap().point.x as f64,
            tex_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(8).unwrap().point.x as f64,
            te_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(7).unwrap().point.x as f64,
            t_width,
            3.0,
        );
        // This should be beginning of second line
        assert_close_to(
            full_layout.hit_test_text_position(6).unwrap().point.x as f64,
            0.0,
            3.0,
        );

        assert_close_to(
            full_layout.hit_test_text_position(3).unwrap().point.x as f64,
            pie_width,
            3.0,
        );

        // This tests that trailing whitespace is included in the first line width.
        assert_close_to(
            full_layout.hit_test_text_position(5).unwrap().point.x as f64,
            piet_space_width,
            3.0,
        );

        // These test y position of text positions on line 1 (0-index)
        assert_close_to(
            full_layout.hit_test_text_position(10).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(9).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(8).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(7).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(6).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );

        // this tests y position of 0 line
        assert_close_to(
            full_layout.hit_test_text_position(5).unwrap().point.y as f64,
            line_zero_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(4).unwrap().point.y as f64,
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
            .new_font_by_name("sans-serif", 15.0) // change this for osx
            .build()
            .unwrap();

        let layout = text_layout
            .new_text_layout(&font, &input[0..3], 30.0)
            .build()
            .unwrap();
        let pie_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..4], 25.0)
            .build()
            .unwrap();
        let piet_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[0..5], 30.0)
            .build()
            .unwrap();
        let piet_space_width = layout.width();

        // "text" should be on second line
        let layout = text_layout
            .new_text_layout(&font, &input[6..10], 25.0)
            .build()
            .unwrap();
        let text_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[6..9], 25.0)
            .build()
            .unwrap();
        let tex_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[6..8], 25.0)
            .build()
            .unwrap();
        let te_width = layout.width();

        let layout = text_layout
            .new_text_layout(&font, &input[6..7], 25.0)
            .build()
            .unwrap();
        let t_width = layout.width();

        let full_layout = text_layout
            .new_text_layout(&font, input, 25.0)
            .build()
            .unwrap();

        println!("lm: {:#?}", full_layout.line_metrics);
        println!("layout width: {:#?}", full_layout.width());

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
        assert_close_to(
            full_layout.hit_test_text_position(10).unwrap().point.x as f64,
            text_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(9).unwrap().point.x as f64,
            tex_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(8).unwrap().point.x as f64,
            te_width,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(7).unwrap().point.x as f64,
            t_width,
            3.0,
        );
        // This should be beginning of second line
        assert_close_to(
            full_layout.hit_test_text_position(6).unwrap().point.x as f64,
            0.0,
            3.0,
        );

        assert_close_to(
            full_layout.hit_test_text_position(3).unwrap().point.x as f64,
            pie_width,
            3.0,
        );

        // This tests that trailing whitespace is included in the first line width.
        assert_close_to(
            full_layout.hit_test_text_position(5).unwrap().point.x as f64,
            piet_space_width,
            3.0,
        );

        // These test y position of text positions on line 1 (0-index)
        assert_close_to(
            full_layout.hit_test_text_position(10).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(9).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(8).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(7).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(6).unwrap().point.y as f64,
            line_one_baseline,
            3.0,
        );

        // this tests y position of 0 line
        assert_close_to(
            full_layout.hit_test_text_position(5).unwrap().point.y as f64,
            line_zero_baseline,
            3.0,
        );
        assert_close_to(
            full_layout.hit_test_text_position(4).unwrap().point.y as f64,
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

        let font = text.new_font_by_name("sans-serif", 12.0).build().unwrap();
        // this should break into four lines
        let layout = text.new_text_layout(&font, input, 30.0).build().unwrap();
        println!("text pos 01: {:?}", layout.hit_test_text_position(0)); // (0.0, 0.0)
        println!("text pos 06: {:?}", layout.hit_test_text_position(5)); // (0.0, 13.9999)
        println!("text pos 11: {:?}", layout.hit_test_text_position(10)); // (0.0, 27.9999)
        println!("text pos 16: {:?}", layout.hit_test_text_position(15)); // (0.0, 41.99999)

        let pt = layout.hit_test_point(Point::new(1.0, -1.0));
        assert_eq!(pt.metrics.text_position, 0);
        assert_eq!(pt.is_inside, true);
        let pt = layout.hit_test_point(Point::new(1.0, 00.0));
        assert_eq!(pt.metrics.text_position, 0);
        let pt = layout.hit_test_point(Point::new(1.0, 04.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(1.0, 18.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(1.0, 32.0));
        assert_eq!(pt.metrics.text_position, 15);

        // over on y axis, but x still affects the text position
        let best_layout = text
            .new_text_layout(&font, "best", std::f64::INFINITY)
            .build()
            .unwrap();
        println!("layout width: {:#?}", best_layout.width()); // 26.0...

        let pt = layout.hit_test_point(Point::new(1.0, 46.0));
        assert_eq!(pt.metrics.text_position, 15);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(25.0, 46.0));
        assert_eq!(pt.metrics.text_position, 19);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(27.0, 46.0));
        assert_eq!(pt.metrics.text_position, 19);
        assert_eq!(pt.is_inside, false);

        // under
        let piet_layout = text
            .new_text_layout(&font, "piet ", std::f64::INFINITY)
            .build()
            .unwrap();
        println!("layout width: {:#?}", piet_layout.width()); // 27.0...

        let pt = layout.hit_test_point(Point::new(1.0, -14.0)); // under
        assert_eq!(pt.metrics.text_position, 0);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(26.0, -14.0)); // under
        assert_eq!(pt.metrics.text_position, 5);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(28.0, -14.0)); // under
        assert_eq!(pt.metrics.text_position, 5);
        assert_eq!(pt.is_inside, false);
    }

    #[test]
    #[cfg(target_os = "macos")]
    // very basic testing that multiline works
    fn test_multiline_hit_test_point_basic() {
        let input = "piet text most best";
        let mut text = CairoText::new();

        let font = text.new_font_by_name("sans-serif", 13.0).build().unwrap();
        // this should break into four lines
        let layout = text.new_text_layout(&font, input, 30.0).build().unwrap();
        println!("text pos 01: {:?}", layout.hit_test_text_position(0)); // (0.0, 0.0)
        println!("text pos 06: {:?}", layout.hit_test_text_position(5)); // (0.0, 12.0)
        println!("text pos 11: {:?}", layout.hit_test_text_position(10)); // (0.0, 24.0)
        println!("text pos 16: {:?}", layout.hit_test_text_position(15)); // (0.0, 36.0)

        let pt = layout.hit_test_point(Point::new(1.0, -1.0));
        assert_eq!(pt.metrics.text_position, 0);
        assert_eq!(pt.is_inside, true);
        let pt = layout.hit_test_point(Point::new(1.0, 00.0));
        assert_eq!(pt.metrics.text_position, 0);
        let pt = layout.hit_test_point(Point::new(1.0, 04.0));
        assert_eq!(pt.metrics.text_position, 5);
        let pt = layout.hit_test_point(Point::new(1.0, 18.0));
        assert_eq!(pt.metrics.text_position, 10);
        let pt = layout.hit_test_point(Point::new(1.0, 32.0));
        assert_eq!(pt.metrics.text_position, 15);

        // over on y axis, but x still affects the text position
        let best_layout = text
            .new_text_layout(&font, "best", std::f64::INFINITY)
            .build()
            .unwrap();
        println!("layout width: {:#?}", best_layout.width()); // 26.0...

        let pt = layout.hit_test_point(Point::new(1.0, 46.0));
        assert_eq!(pt.metrics.text_position, 15);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(25.0, 46.0));
        assert_eq!(pt.metrics.text_position, 19);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(27.0, 46.0));
        assert_eq!(pt.metrics.text_position, 19);
        assert_eq!(pt.is_inside, false);

        // under
        let piet_layout = text
            .new_text_layout(&font, "piet ", std::f64::INFINITY)
            .build()
            .unwrap();
        println!("layout width: {:#?}", piet_layout.width()); // ???

        let pt = layout.hit_test_point(Point::new(1.0, -14.0)); // under
        assert_eq!(pt.metrics.text_position, 0);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(25.0, -14.0)); // under
        assert_eq!(pt.metrics.text_position, 5);
        assert_eq!(pt.is_inside, false);

        let pt = layout.hit_test_point(Point::new(27.0, -14.0)); // under
        assert_eq!(pt.metrics.text_position, 5);
        assert_eq!(pt.is_inside, false);
    }
}
