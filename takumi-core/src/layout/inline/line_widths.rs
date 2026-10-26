//! Blink's [`LineWidths`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/line_widths.cc),
//! without shapes, ruby, or initial letters.

use std::ops::Index;

use super::{
  inline_box_state::InlineBoxState, inline_break_token::InlineBreakToken,
  inline_item::InlineItemType, inline_node::InlineNode, inline_node_data::InlineNodeData,
};
use crate::{
  fonts::shaping::shape_result_view::ShapeResultView,
  layout::exclusions::layout_opportunity::LayoutOpportunity, layout_unit::LayoutUnit,
  style::VerticalAlign,
};

/// Blink's `LineWidths`: each line's available width, for at most one float beside the first
/// lines, when every line has the same height.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LineWidths {
  pub(crate) default_width: LayoutUnit,
  excluded_width: LayoutUnit,
  num_excluded_lines: usize,
}

impl LineWidths {
  /// Blink's `Set`: none when the lines' widths need layout to know.
  pub(crate) fn new(
    node: InlineNode<'_>,
    items_data: &InlineNodeData<'_>,
    opportunities: &[LayoutOpportunity],
    break_token: Option<&InlineBreakToken>,
  ) -> Option<Self> {
    let first_opportunity = opportunities.first()?;

    if opportunities.len() == 1 && !items_data.has_floats {
      return Some(Self {
        default_width: first_opportunity.rect.inline_size(),
        ..Self::default()
      });
    }

    // Only one simple exclusion.
    if opportunities.len() > 2 {
      return None;
    }

    // The strut's metrics, which every line has when the block uses one font.
    // <https://drafts.csswg.org/css2/visudet.html#strut>
    let block_context = &node.block.context;
    let fonts = &items_data.fonts;
    let mut line_box = InlineBoxState::new(0, block_context, fonts);

    line_box.compute_text_metrics(None);

    let primary_font = line_box.font.primary_font()?;
    let mut items = &items_data.items[..];
    let mut is_empty_so_far = true;

    if let Some(break_token) = break_token {
      items = &items[break_token.start.item_index..];
      is_empty_so_far = false;
    }

    for item in items {
      match item.item_type {
        InlineItemType::Text => {
          let Some(shape_result) = item.shape_result.as_ref().filter(|_| item.length() != 0) else {
            continue;
          };

          // Another font may make the line taller than the strut.
          if shape_result.has_fallback_fonts(&primary_font) {
            let object = item.layout_object?;
            let mut text_box = InlineBoxState::new(0, object.context(), fonts);

            text_box.compute_text_metrics(None);

            if text_box.include_used_fonts {
              text_box
                .accumulate_used_fonts(&ShapeResultView::create_from_result(shape_result), 1.0);
            }

            if !line_box.metrics.contains(&text_box.metrics) {
              return None;
            }
          }
        }
        InlineItemType::OpenTag => {
          if item
            .style()
            .is_some_and(|style| style.base_data.vertical_align != VerticalAlign::default())
          {
            return None;
          }
        }
        InlineItemType::CloseTag
        | InlineItemType::Control
        | InlineItemType::OutOfFlowPositioned
        | InlineItemType::BidiControl => {}
        // Only leading floats size without layout.
        InlineItemType::Floating => {
          if !is_empty_so_far {
            return None;
          }
        }
        InlineItemType::AtomicInline
        | InlineItemType::BlockInInline
        | InlineItemType::ListMarker => {
          return None;
        }
      }

      if is_empty_so_far && !item.is_empty_item {
        is_empty_so_far = false;
      }
    }

    // Floats only before `break_token`, or leading floats of no size, exclude nothing.
    if opportunities.len() == 1 {
      return Some(Self {
        default_width: first_opportunity.rect.inline_size(),
        ..Self::default()
      });
    }

    let line_height = line_box.metrics.line_height();

    if line_height <= LayoutUnit::ZERO {
      return None;
    }

    let last_opportunity = opportunities.last()?;
    let exclusion_block_size =
      last_opportunity.rect.block_start_offset() - first_opportunity.rect.block_start_offset();
    // `LayoutUnit` division is too coarse: 46.25 / 23 is 2.
    let num_excluded_lines = (exclusion_block_size.to_float() / line_height.to_float()).ceil();

    Some(Self {
      default_width: last_opportunity.rect.inline_size(),
      excluded_width: first_opportunity.rect.inline_size(),
      num_excluded_lines: num_excluded_lines as usize,
    })
  }
}

impl Index<usize> for LineWidths {
  type Output = LayoutUnit;

  fn index(&self, index: usize) -> &LayoutUnit {
    if index < self.num_excluded_lines {
      &self.excluded_width
    } else {
      &self.default_width
    }
  }
}
