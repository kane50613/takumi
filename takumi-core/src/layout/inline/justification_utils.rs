//! Blink's [`justification_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/justification_utils.cc),
//! without ruby, SVG text, or `text-combine-upright`.

use std::rc::Rc;

use super::{
  inline_item::InlineItemType,
  inline_item_result::{InlineItemResult, InlineItemResults},
  line_info::LineInfo,
};
use crate::{
  fonts::shaping::{
    shape_result_spacing::{ExpansionSetup, ShapeResultSpacing},
    shape_result_view::ShapeResultView,
  },
  layout_unit::{FixedPoint, InlineLayoutUnit, LayoutUnit, TextRunLayoutUnit},
  style::Direction as TextDirection,
  text::{character::OBJECT_REPLACEMENT_CHARACTER, text_justify::TextJustify},
};

/// Blink's `GetTextJustify`: items without a box, such as controls, take no expansion.
fn text_justify(item_result: &InlineItemResult<'_, '_>) -> TextJustify {
  if item_result.item.layout_object.is_none() {
    return TextJustify::None;
  }

  TextJustify::Auto
}

/// Blink's `SetupItemJustificationOpportunity`.
fn setup_item_justification_opportunity(
  item_result: &InlineItemResult<'_, '_>,
  text: &str,
  end_offset: usize,
  base_direction: TextDirection,
  spacing_setup: &mut ExpansionSetup<'_, '_>,
) {
  if item_result.start_offset() >= end_offset || item_result.has_only_pre_wrap_trailing_spaces {
    return;
  }

  let method = text_justify(item_result);

  if let Some(shape_result) = &item_result.shape_result {
    let start_index = shape_result.start_index() as usize;
    let end_index = (shape_result.end_index() as usize).min(end_offset);

    spacing_setup.count_opportunities(method, &text[start_index..end_index], base_direction);
  } else if item_result.item.item_type == InlineItemType::AtomicInline {
    spacing_setup.count_opportunity(method, OBJECT_REPLACEMENT_CHARACTER);
  }
}

/// Blink's `SetupJustificationOpportunity`, without serializing the line's text.
fn setup_justification_opportunity(
  results: &InlineItemResults<'_, '_>,
  text: &str,
  end_offset: usize,
  base_direction: TextDirection,
  spacing_setup: &mut ExpansionSetup<'_, '_>,
) {
  let Some(last) = results.last() else {
    return;
  };
  let count_hyphen = |spacing_setup: &mut ExpansionSetup<'_, '_>| {
    if let Some(hyphen) = &last.hyphen {
      spacing_setup.count_opportunities(text_justify(last), &hyphen.text, base_direction);
    }
  };

  // Justification comes before bidi reordering, so an RTL line counts from its end.
  if base_direction == TextDirection::Rtl {
    count_hyphen(spacing_setup);

    for item_result in results.iter().rev() {
      setup_item_justification_opportunity(
        item_result,
        text,
        end_offset,
        base_direction,
        spacing_setup,
      );
    }
  } else {
    for item_result in results {
      setup_item_justification_opportunity(
        item_result,
        text,
        end_offset,
        base_direction,
        spacing_setup,
      );
    }

    count_hyphen(spacing_setup);
  }
}

/// Blink's `JustifyResults`: spreads the expansion over the results. Returns the spacing after the
/// last glyph, zero after an atomic inline.
fn justify_results(
  spacing: &mut ShapeResultSpacing<'_>,
  results: &mut InlineItemResults<'_, '_>,
) -> TextRunLayoutUnit {
  let mut last_glyph_spacing = TextRunLayoutUnit::ZERO;

  for item_result in results.iter_mut() {
    if item_result.has_only_pre_wrap_trailing_spaces {
      break;
    }

    let method = text_justify(item_result);

    if let Some(shape_result) = &item_result.shape_result {
      let mut shape_result = shape_result.create_shape_result();
      let text_start_offset = item_result.start_offset() as i32 - shape_result.start_index() as i32;

      last_glyph_spacing = shape_result.apply_expansion(method, spacing, text_start_offset);
      item_result.inline_size = shape_result.snapped_width();

      if item_result.is_hyphenated
        && let Some(hyphen) = &item_result.hyphen
      {
        item_result.inline_size += hyphen.inline_size();
      }

      item_result.shape_result = Some(Rc::new(ShapeResultView::create_from_result(&Rc::new(
        shape_result,
      ))));
    } else if item_result.item.item_type == InlineItemType::AtomicInline {
      let (spacing_before, spacing_after) =
        spacing.compute_expansion_for(method, OBJECT_REPLACEMENT_CHARACTER);

      last_glyph_spacing = TextRunLayoutUnit::ZERO;
      item_result.inline_size += (spacing_before + spacing_after).to::<LayoutUnit>();
      item_result.spacing_before = spacing_before.to::<LayoutUnit>();
    }
  }

  last_glyph_spacing
}

/// Blink's `ApplyJustification`: justifies `line_info` into `space`. Returns how far the line moves,
/// or `None` when it cannot justify and aligns to the start instead.
pub(crate) fn apply_justification(
  space: LayoutUnit,
  line_info: &mut LineInfo<'_, '_>,
) -> Option<LayoutUnit> {
  // Empty lines and lines that overflow align to the start.
  if line_info.is_empty_line || space <= LayoutUnit::ZERO {
    return None;
  }

  // Justify up to the end of the visible text, ignoring preserved trailing spaces.
  let end_offset = line_info.end_offset_for_justify();

  if end_offset == line_info.start_offset() {
    return None;
  }

  // Cut at `end_offset`, as `ShapeResult::apply_expansion` reads past it.
  let text_content = line_info.items_data.text_content.as_str();
  let line_text = &text_content[..end_offset];

  if line_text.is_empty() {
    return None;
  }

  let mut spacing = ShapeResultSpacing::new(line_text, false);

  {
    let mut spacing_setup = spacing.expansion_setup(InlineLayoutUnit::from(space), false, false);

    setup_justification_opportunity(
      &line_info.results,
      line_text,
      end_offset,
      line_info.base_direction,
      &mut spacing_setup,
    );
  }

  if !spacing.has_expansion() {
    return None;
  }

  justify_results(&mut spacing, &mut line_info.results);

  Some(LayoutUnit::ZERO)
}
