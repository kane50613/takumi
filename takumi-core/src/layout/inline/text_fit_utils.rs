//! Blink's [`text_fit_utils.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/text_fit_utils.cc),
//! scaling `text-fit` text at paint time, as stable Blink does, and reshaping it at a scaled font
//! size only around fixed `letter-spacing` or `word-spacing`. Without a minimum font size, which
//! takumi has no setting for, or the han kerning and text-autospace takumi does not shape.

use std::rc::Rc;

use super::{
  inline_item::{InlineItem, InlineItemType},
  inline_layout_algorithm::LineBoxFragment,
  inline_node::InlineNode,
  inline_node_data::InlineNodeData,
  line_info::LineInfo,
  logical_line_item::LogicalLineItem,
  text_fit_scale::TextFitScale,
};
use crate::{
  context::RenderContext,
  fonts::{
    font::Font,
    font_description::FontDescription,
    shaping::{
      harfbuzz_shaper::HarfBuzzShaper, shape_result::ShapeResult,
      shape_result_spacing::ShapeResultSpacing, shape_result_view::ShapeResultView,
    },
  },
  layout_unit::LayoutUnit,
  style::{TextFitMode, TextFitTarget},
};

/// Blink's `2.0 * DevicePixelRatio()`, at takumi's one device pixel per CSS pixel.
const EPSILON: LayoutUnit = LayoutUnit::from_int(2);

/// Blink's `ParagraphScale`: the scale every line of a consistent `text-fit` takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ParagraphScale {
  /// The paint-time scale, or the font scale.
  pub(crate) scale: f32,
  /// The paint-time scale after the font scale.
  pub(crate) additional_paint_time_scale: f32,
}

impl Default for ParagraphScale {
  fn default() -> Self {
    Self {
      scale: 1.0,
      additional_paint_time_scale: 1.0,
    }
  }
}

/// Blink's `ScaleLine`: scales the line's text at paint time, and returns whether the line height
/// scales with it.
fn scale_line(scale_factor: f32, line_info: &mut LineInfo<'_, '_>) -> bool {
  let mut should_scale_line_height = false;
  let mut inline_size = line_info.text_indent;

  for item in &mut line_info.results {
    if item.item.item_type != InlineItemType::Text && !item.item.is_forced_line_break() {
      inline_size += item.inline_size;

      continue;
    }

    let text_fit_scale = item
      .text_fit_scale
      .get_or_insert_with(TextFitScale::default);

    text_fit_scale.scale = scale_factor;

    if text_fit_scale.scale != 1.0 {
      should_scale_line_height = true;
    }

    inline_size = LayoutUnit::from_float(
      inline_size.to_float() + item.inline_size.to_float() * text_fit_scale.scale,
    );
  }

  line_info.set_width(line_info.available_width(), inline_size);
  line_info.text_fit_scale = scale_factor;

  should_scale_line_height
}

/// The shaper and spacing `text-fit` reshapes a paragraph's text with.
struct FitShaper<'a, 'c> {
  items_data: &'a InlineNodeData<'c>,
  shaper: HarfBuzzShaper<'a>,
  spacing: ShapeResultSpacing<'a>,
}

impl<'a, 'c> FitShaper<'a, 'c> {
  fn new(items_data: &'a InlineNodeData<'c>) -> Self {
    Self {
      items_data,
      shaper: HarfBuzzShaper::new(&items_data.text_content),
      spacing: ShapeResultSpacing::new(&items_data.text_content, false),
    }
  }

  /// Blink's `ShapeForFit` of `start_offset..end_offset` of `item` with `font`, then the spacing
  /// of `spacing_description`: the shape, and its width before the spacing.
  fn shape(
    &mut self,
    item: &InlineItem<'_>,
    start_offset: usize,
    end_offset: usize,
    font: &Font,
    spacing_description: &FontDescription,
  ) -> (ShapeResult, LayoutUnit) {
    let mut shape_result = match &self.items_data.segments {
      Some(segments) => segments.shape_text(
        &self.shaper,
        font,
        item.direction(),
        start_offset,
        end_offset,
        item.index,
      ),
      None => {
        let mut range = item.create_run_segmenter_range();

        range.end = end_offset;
        self
          .shaper
          .shape_pre_segmented(font, item.direction(), start_offset, end_offset, &range)
      }
    };
    let size_without_spacing = shape_result.snapped_width().clamp_negative_to_zero();

    if self.spacing.set_spacing_from(
      spacing_description.letter_spacing,
      spacing_description.word_spacing,
    ) {
      shape_result.apply_spacing(&mut self.spacing, 0);
    }

    (shape_result, size_without_spacing)
  }
}

/// Blink's `HasFixedSpacing`: a nonzero length `letter-spacing` or `word-spacing`.
fn has_fixed_spacing(font_description: &FontDescription) -> bool {
  (font_description.is_letter_spacing_fixed && font_description.letter_spacing != 0.0)
    || (font_description.is_word_spacing_fixed && font_description.word_spacing != 0.0)
}

/// Blink's `PercentageSpacingDescription`: `font_description` without its fixed spacing.
fn percentage_spacing_description(font_description: &FontDescription) -> FontDescription {
  let mut copy = font_description.clone();

  if copy.is_letter_spacing_fixed {
    copy.letter_spacing = 0.0;
  }

  if copy.is_word_spacing_fixed {
    copy.word_spacing = 0.0;
  }

  copy
}

/// Blink's `RestrictScale`: `scale` within `limit`, one way or the other.
fn restrict_scale(scale: f32, is_grow: bool, limit: Option<f32>) -> f32 {
  let Some(limit) = limit else {
    return scale;
  };

  if is_grow {
    scale.min(limit.max(1.0))
  } else {
    scale.max(limit.min(1.0))
  }
}

/// Blink's `ScaledFontDescription`, as a font.
fn scaled_font(font: &Font, scale_factor: f32, context: &RenderContext) -> Rc<Font> {
  let mut scaled_description = font.description.clone();

  scaled_description.fallback.computed_size *= scale_factor;

  Rc::new(context.font(scaled_description))
}

/// The text fragments of a line box, forced breaks and hidden ones among them, as Blink's cursor
/// visits the inline leaves of its fragment items, each with the item holding its text.
fn line_texts<'l, 'a, 'c>(
  line: &'l LineBoxFragment<'a, 'c>,
  items: &'a [InlineItem<'c>],
) -> impl Iterator<Item = (&'a InlineItem<'c>, &'l LogicalLineItem<'a, 'c>)> {
  line
    .items
    .children
    .iter()
    .filter(|child| child.shape_result.is_some())
    .filter_map(|child| {
      let item = child.inline_item.or_else(|| {
        items.iter().find(|item| {
          item.start_offset() <= child.text_offset.start
            && child.text_offset.end <= item.end_offset()
        })
      })?;

      Some((item, child))
    })
}

/// Blink's `ComputeAdditionalPaintTimeScale`: the paint-time scale that fills the line after its
/// text reshapes at `scale`.
fn compute_additional_paint_time_scale(
  fit_shaper: &mut FitShaper<'_, '_>,
  available_width: LayoutUnit,
  line: &LineBoxFragment<'_, '_>,
  scale: f32,
  static_total_size: LayoutUnit,
  context: &RenderContext,
) -> f32 {
  let mut flexible_total_size = LayoutUnit::ZERO;

  for (item, child) in line_texts(line, &fit_shaper.items_data.items) {
    if item.is_forced_line_break() {
      continue;
    }

    let Some(object) = item.layout_object else {
      continue;
    };
    let font = scaled_font(
      &fit_shaper.items_data.fonts.get(object.context()),
      scale,
      context,
    );
    let (shape_result, _) = fit_shaper.shape(
      item,
      child.text_offset.start,
      child.text_offset.end,
      &font,
      &font.description,
    );

    flexible_total_size += shape_result.snapped_width().clamp_negative_to_zero();
  }

  let remaining_space = available_width - (flexible_total_size + static_total_size);

  if remaining_space.abs() >= EPSILON {
    (flexible_total_size + remaining_space).to_float() / flexible_total_size.to_float()
  } else {
    1.0
  }
}

impl<'c> InlineNode<'c> {
  /// Blink's `ShouldApplyTextFit`: `text-fit` leaves a paragraph with a float alone.
  pub(crate) fn should_apply_text_fit(self, items_data: &InlineNodeData<'c>) -> bool {
    self.block.context.style.misc_inherited1_data.text_fit.mode != TextFitMode::None
      && !items_data.has_floats
  }

  /// Blink's `MeasurePerBlockScale`: the scale the narrowest-fitting line of `lines` takes, for a
  /// consistent `text-fit`.
  pub(crate) fn measure_per_block_scale(
    self,
    items_data: &InlineNodeData<'c>,
    lines: &[LineBoxFragment<'_, 'c>],
    available_width: LayoutUnit,
  ) -> ParagraphScale {
    let context = &self.block.context;
    let mut minimum_scale = f32::INFINITY;
    let text_fit = context.style.misc_inherited1_data.text_fit;

    if text_fit.target != TextFitTarget::Consistent {
      return ParagraphScale::default();
    }

    let mut additional_paint_time_scale = 1.0;
    let mut fit_shaper = FitShaper::new(items_data);

    for line in lines {
      let remaining_space = available_width - line.inline_size;

      if remaining_space.abs() < EPSILON {
        continue;
      }

      let mut flexible_total_size = LayoutUnit::ZERO;
      let mut flexible_total_size_including_letter_spacing = LayoutUnit::ZERO;
      let mut did_reshape = false;

      for (item, child) in line_texts(line, &fit_shaper.items_data.items) {
        if item.is_forced_line_break() {
          continue;
        }

        let Some(object) = item.layout_object else {
          continue;
        };
        let font = items_data.fonts.get(object.context());

        if has_fixed_spacing(&font.description) {
          let (nospacing_shape, _) = fit_shaper.shape(
            item,
            child.text_offset.start,
            child.text_offset.end,
            &font,
            &percentage_spacing_description(&font.description),
          );

          did_reshape = true;
          flexible_total_size += nospacing_shape.snapped_width().clamp_negative_to_zero();
        } else {
          flexible_total_size += child.inline_size;
        }

        flexible_total_size_including_letter_spacing += child.inline_size;
      }

      if flexible_total_size == LayoutUnit::ZERO
        || remaining_space + flexible_total_size <= LayoutUnit::ZERO
      {
        continue;
      }

      let scale =
        (remaining_space + flexible_total_size).to_float() / flexible_total_size.to_float();

      if scale < minimum_scale {
        minimum_scale = scale;

        // Paint-time scaling scales the letter spacing too, so it stays out of the flexible
        // size.
        if did_reshape {
          let static_total_size =
            available_width - remaining_space - flexible_total_size_including_letter_spacing;

          additional_paint_time_scale = compute_additional_paint_time_scale(
            &mut fit_shaper,
            available_width,
            line,
            scale,
            static_total_size,
            context,
          );
        }
      }
    }

    ParagraphScale {
      scale: if minimum_scale.is_finite() {
        restrict_scale(
          minimum_scale,
          text_fit.mode == TextFitMode::Grow,
          text_fit.limit,
        )
      } else {
        1.0
      },
      additional_paint_time_scale,
    }
  }
}

/// Blink's `LineFitter`: scales a line's text to fill it.
pub(crate) struct LineFitter<'l, 'a, 'c> {
  node: InlineNode<'c>,
  line_info: &'l mut LineInfo<'a, 'c>,
  items_data: &'a InlineNodeData<'c>,
  fit_shaper: FitShaper<'a, 'c>,
}

impl<'l, 'a, 'c> LineFitter<'l, 'a, 'c> {
  pub(crate) fn new(node: InlineNode<'c>, line_info: &'l mut LineInfo<'a, 'c>) -> Self {
    let items_data = line_info.items_data;

    Self {
      node,
      line_info,
      items_data,
      fit_shaper: FitShaper::new(items_data),
    }
  }

  /// Blink's `MeasureScale`: the scale that fills the line, infinite for a line without text.
  fn measure_scale(&mut self) -> f32 {
    let original_width = self.line_info.width();
    let container_width = self.line_info.available_width();
    let diff = container_width - original_width;

    if diff.abs() < EPSILON {
      return 1.0;
    }

    let text_fit = self.node.block.context.style.misc_inherited1_data.text_fit;
    let target = text_fit.target;
    let apply_text_grow = text_fit.mode == TextFitMode::Grow && target != TextFitTarget::Consistent;
    let apply_text_shrink =
      text_fit.mode == TextFitMode::Shrink && target != TextFitTarget::Consistent;

    if (diff > LayoutUnit::ZERO && !apply_text_grow)
      || (diff < LayoutUnit::ZERO && !apply_text_shrink)
    {
      return 1.0;
    }

    if target == TextFitTarget::PerLine && self.line_info.is_last_line {
      return 1.0;
    }

    // The static parts and the flexible parts of the line.
    let mut static_total_size = LayoutUnit::ZERO;
    let mut flexible_total_size = LayoutUnit::ZERO;

    for item in &self.line_info.results {
      if item.item.item_type != InlineItemType::Text {
        static_total_size += item.inline_size;

        continue;
      }

      let Some(object) = item.item.layout_object else {
        continue;
      };
      let font = self.items_data.fonts.get(object.context());

      if has_fixed_spacing(&font.description) {
        let (nospacing_shape, _) = self.fit_shaper.shape(
          item.item,
          item.start_offset(),
          item.end_offset(),
          &font,
          &percentage_spacing_description(&font.description),
        );
        let size = nospacing_shape.snapped_width().clamp_negative_to_zero();

        flexible_total_size += size;
        static_total_size += item.inline_size - size;
      } else {
        flexible_total_size += item.inline_size;
      }
    }

    if flexible_total_size <= LayoutUnit::ZERO {
      return f32::INFINITY;
    }

    let scale_factor =
      (container_width - static_total_size).to_float() / flexible_total_size.to_float();

    restrict_scale(
      scale_factor,
      text_fit.mode == TextFitMode::Grow,
      text_fit.limit,
    )
  }

  /// Blink's `FitLine`, returning whether the line height scales.
  pub(crate) fn fit_line(
    &mut self,
    scale_factor: f32,
    additional_paint_time_scale: Option<f32>,
  ) -> bool {
    let is_grow = scale_factor > 1.0;
    let context = &self.node.block.context;
    let text_fit = context.style.misc_inherited1_data.text_fit;
    let has_fixed_spacing = self.line_info.results.iter().any(|item| {
      item.item.item_type == InlineItemType::Text
        && item.item.layout_object.is_some_and(|object| {
          has_fixed_spacing(&self.items_data.fonts.get(object.context()).description)
        })
    });

    // Stable Blink's `TextFitMethod::kScale`.
    if !has_fixed_spacing {
      return scale_line(scale_factor, self.line_info);
    }

    let mut static_total_size = LayoutUnit::ZERO;
    let mut flexible_total_size = LayoutUnit::ZERO;

    for item in &mut self.line_info.results {
      let Some(object) = item.item.layout_object else {
        static_total_size += item.inline_size;

        continue;
      };
      let font = self.items_data.fonts.get(object.context());

      if item.item.item_type != InlineItemType::Text {
        if item.item.is_forced_line_break() {
          item.text_fit_scale = Some(TextFitScale {
            scale: 1.0,
            font: Some(scaled_font(&font, scale_factor, context)),
          });
        }

        static_total_size += item.inline_size;

        continue;
      }

      let scaled_font = scaled_font(&font, scale_factor, context);
      let (shape_result, size_without_spacing) = self.fit_shaper.shape(
        item.item,
        item.start_offset(),
        item.end_offset(),
        &scaled_font,
        &scaled_font.description,
      );

      item.inline_size = shape_result.snapped_width().clamp_negative_to_zero();
      static_total_size += item.inline_size - size_without_spacing;

      item.shape_result = Some(Rc::new(ShapeResultView::create_from_result(&Rc::new(
        shape_result,
      ))));
      item.text_fit_scale = Some(TextFitScale {
        scale: 1.0,
        font: Some(scaled_font),
      });
      flexible_total_size += size_without_spacing;
    }

    // The paint-time scale that fills the line after the font scale.
    let mut total_scale = scale_factor;
    let paint_scale_limit = text_fit.limit.map(|limit| limit / total_scale);

    if let Some(additional_paint_time_scale) = additional_paint_time_scale {
      // A consistent `text-fit`.
      if additional_paint_time_scale != 1.0 {
        let paint_scale = restrict_scale(additional_paint_time_scale, is_grow, paint_scale_limit);

        scale_line(paint_scale, self.line_info);
        total_scale = paint_scale;
      }
    } else {
      // A per-line `text-fit`.
      let container_width = self.line_info.available_width();

      if (container_width - self.line_info.compute_width()).abs() >= EPSILON {
        let paint_scale =
          (container_width - static_total_size).to_float() / flexible_total_size.to_float();
        let paint_scale = restrict_scale(paint_scale, is_grow, paint_scale_limit);

        scale_line(paint_scale, self.line_info);
        total_scale *= paint_scale;
      }
    }

    let available_width = self.line_info.available_width();
    let width = self.line_info.compute_width();

    self.line_info.set_width(available_width, width);
    self.line_info.text_fit_scale = total_scale;

    true
  }

  /// Blink's `MeasureAndFitLine`.
  pub(crate) fn measure_and_fit_line(&mut self) -> bool {
    let scale_factor = self.measure_scale();

    scale_factor.is_finite() && scale_factor != 1.0 && self.fit_line(scale_factor, None)
  }
}

#[cfg(test)]
mod tests {
  use crate::layout::{
    inline::testing::{read_lines, render_tree, span},
    node::Node,
  };

  /// A text item's scale, font size, and inline size.
  type FittedText = (f32, f32, f32);

  /// Each line's `text-fit` scale and text items.
  fn fitted_lines(text: &str, css: &str, available_width: f32) -> Vec<(f32, Vec<FittedText>)> {
    let block = render_tree([Node::text(text)], css);

    read_lines(&block, available_width, |_, lines| {
      lines
        .iter()
        .map(|line| {
          let items = line
            .items
            .children
            .iter()
            .filter(|item| item.shape_result.is_some())
            .map(|item| {
              let scale = item.text_fit_scale.as_ref();

              (
                scale.map_or(1.0, |scale| scale.scale),
                scale
                  .and_then(|scale| scale.font.as_ref())
                  .map_or(10.0, |font| font.description.fallback.computed_size),
                item.inline_size.to_float(),
              )
            })
            .collect();

          (line.text_fit_scale, items)
        })
        .collect()
    })
  }

  /// A line grows at paint time to fill its width.
  #[test]
  fn a_line_grows_to_its_width() {
    assert_eq!(
      fitted_lines("aaa", ".container { text-fit: grow per-line-all }", 36.0),
      [(2.0, vec![(2.0, 10.0, 36.0)])]
    );
  }

  /// A grown line's height scales with its text, `line-height: 1` included.
  #[test]
  fn a_grown_line_is_taller() {
    let block = render_tree(
      [Node::text("aaa")],
      ".container { text-fit: grow per-line-all }",
    );
    let block_size = read_lines(&block, 36.0, |_, lines| lines[0].block_size);

    assert_eq!(block_size.to_float(), 20.0);
  }

  /// A reshaped text's unitless `line-height` multiplies its scaled font size.
  #[test]
  fn a_reshaped_line_height_takes_the_scaled_font_size() {
    let block = render_tree(
      [span("aaa")],
      ".container { text-fit: grow per-line-all; letter-spacing: 1px } .span { line-height: 3 }",
    );
    let block_size = read_lines(&block, 42.0, |_, lines| lines[0].block_size);

    assert!(
      (block_size.to_float() - 3.0 * 10.0 * 39.0 / 18.0).abs() < 0.1,
      "{block_size:?}"
    );
  }

  /// An overflowing line shrinks.
  #[test]
  fn a_line_shrinks_to_its_width() {
    assert_eq!(
      fitted_lines(
        "aaaaaa",
        ".container { text-fit: shrink per-line-all; white-space: nowrap }",
        18.0
      ),
      [(0.5, vec![(0.5, 10.0, 18.0)])]
    );
  }

  /// `per-line` leaves the last line as it is.
  #[test]
  fn per_line_leaves_the_last_line() {
    assert_eq!(
      fitted_lines("aaa", ".container { text-fit: grow per-line }", 36.0),
      [(1.0, vec![(1.0, 10.0, 18.0)])]
    );
  }

  /// `consistent` scales every line by the scale of the line that grows least.
  #[test]
  fn consistent_lines_share_a_scale() {
    assert_eq!(
      fitted_lines("aaaa bb", ".container { text-fit: grow consistent }", 30.0),
      [
        (1.25, vec![(1.25, 10.0, 30.0)]),
        (1.25, vec![(1.25, 10.0, 15.0)])
      ]
    );
  }

  /// A fixed `letter-spacing`, which paint-time scaling would grow, reshapes the text at a larger
  /// font size.
  #[test]
  fn fixed_letter_spacing_reshapes_at_a_scaled_font_size() {
    let lines = fitted_lines(
      "aaa",
      ".container { text-fit: grow per-line-all; letter-spacing: 1px }",
      42.0,
    );
    let (line_scale, items) = &lines[0];
    let (scale, font_size, inline_size) = items[0];

    assert_eq!(scale, 1.0);
    assert!((font_size - 10.0 * 39.0 / 18.0).abs() < 0.01, "{font_size}");
    assert!((inline_size - 42.0).abs() < 2.0, "{inline_size}");
    assert!((line_scale - 39.0 / 18.0).abs() < 0.01, "{line_scale}");
  }
}
