//! Blink's [`InlineNode::ComputeMinMaxSizes`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_node.cc),
//! without initial letters, ruby, SVG text, or `max-content-sizing`. The max-content size takes a
//! second pass where Blink derives it from the min-content pass, which it checks gives the same
//! size.

use taffy::{Clear, Float};

use super::{
  inline_item::{InlineItemType, LayoutObject},
  inline_item_result::LineBoxStrut,
  inline_node::InlineNode,
  inline_node_data::InlineNodeData,
  leading_floats::LeadingFloats,
  line_breaker::{AtomicInlineLayout, LineBreaker, LineBreakerMode},
  line_info::LineInfo,
};
use crate::{
  layout::{
    constraint_space::ConstraintSpace,
    exclusions::{exclusion_space::ExclusionSpace, layout_opportunity::LineLayoutOpportunity},
    min_max_sizes::MinMaxSizes,
    tree::RenderNode,
  },
  layout_unit::LayoutUnit,
  style::Direction as TextDirection,
};

/// Blink's `InlineNode::FloatingObject`: a float on a line, by its margin box's max-content size.
struct FloatingObject {
  float: Float,
  clear: Clear,
  inline_max_size_with_margin: LayoutUnit,
}

/// Blink's `FloatsMaxSize`: the floats beside each line in max-content.
#[derive(Default)]
struct FloatsMaxSize {
  floats_inline_size: LayoutUnit,
  floating_objects: Vec<FloatingObject>,
}

impl FloatsMaxSize {
  /// Blink's `ComputeMaxSizeForLine`: a line's size with the floats beside it. A float that clears
  /// the one before it starts a "line" of its own, unlike block layout, which resets only the
  /// side it clears.
  fn compute_max_size_for_line(
    &mut self,
    line_inline_size: LayoutUnit,
    mut max_inline_size: LayoutUnit,
  ) -> LayoutUnit {
    if self.floating_objects.is_empty() {
      return max_inline_size.max(line_inline_size);
    }

    let mut previous_float_type = Float::None;

    for floating_object in &self.floating_objects {
      let clears_previous = matches!(
        (previous_float_type, floating_object.clear),
        (Float::Left, Clear::Both | Clear::Left) | (Float::Right, Clear::Both | Clear::Right)
      );

      if clears_previous {
        max_inline_size = max_inline_size.max(line_inline_size + self.floats_inline_size);
        self.floats_inline_size = LayoutUnit::ZERO;
      }

      // A float that negative margins move outside the content box takes no room.
      self.floats_inline_size += floating_object
        .inline_max_size_with_margin
        .clamp_negative_to_zero();
      previous_float_type = floating_object.float;
    }

    max_inline_size = max_inline_size.max(line_inline_size + self.floats_inline_size);
    self.floats_inline_size = LayoutUnit::ZERO;
    self.floating_objects.clear();
    max_inline_size
  }
}

impl<'c> InlineNode<'c> {
  /// Blink's `ComputeMinMaxSizes`: the min-content size, the widest line when every opportunity
  /// breaks, and the max-content size, the widest line when only forced breaks do. Kept for text
  /// alone, as Blink keeps a box's intrinsic sizes until its content changes.
  pub(crate) fn compute_min_max_sizes(
    self,
    items_data: &InlineNodeData<'c>,
    atomic_inline: &mut dyn AtomicInlineLayout<'c>,
  ) -> MinMaxSizes {
    if let Some(sizes) = items_data.min_max_sizes.get() {
      return sizes;
    }

    let min_size =
      self.compute_content_size(items_data, LineBreakerMode::MinContent, atomic_inline);
    let max_size =
      self.compute_content_size(items_data, LineBreakerMode::MaxContent, atomic_inline);
    // A negative `text-indent` can make the min-content size the larger.
    let sizes = MinMaxSizes {
      min_size,
      max_size: max_size.max(min_size),
    };

    if items_data.holds_only_text() {
      items_data.min_max_sizes.set(Some(sizes));
    }
    sizes
  }

  /// Blink's `ComputeContentSize`: the widest line, with its floats, when the lines break as
  /// `mode` says.
  fn compute_content_size(
    self,
    items_data: &InlineNodeData<'c>,
    mode: LineBreakerMode,
    atomic_inline: &mut dyn AtomicInlineLayout<'c>,
  ) -> LayoutUnit {
    let direction = self.block.context.style.inherited_data.direction;
    let available_inline_size = match mode {
      LineBreakerMode::MinContent => LayoutUnit::ZERO,
      LineBreakerMode::MaxContent | LineBreakerMode::Content => LayoutUnit::MAX,
    };
    let space = ConstraintSpace::new(None, direction);
    let leading_floats = LeadingFloats::default();
    let mut exclusion_space = ExclusionSpace::default();
    // Each line's inline size and the floats on it, which take no part in breaking it.
    let mut lines: Vec<(LayoutUnit, Vec<&'c RenderNode>)> = Vec::new();
    let mut line_breaker = LineBreaker::new(
      self,
      items_data,
      mode,
      space,
      LineLayoutOpportunity::new(available_inline_size),
      &leading_floats,
      None,
      &mut *atomic_inline,
      &mut exclusion_space,
    );

    loop {
      let mut line_info = LineInfo::new(items_data, &self.block.context);

      line_breaker.next_line(&mut line_info);

      if line_info.results.is_empty() {
        break;
      }

      let floats = line_info
        .results
        .iter()
        .filter(|item_result| item_result.item.item_type == InlineItemType::Floating)
        .filter_map(|item_result| item_result.item.layout_object.map(LayoutObject::node))
        .collect();

      lines.push((line_info.width(), floats));

      if line_breaker.is_finished() {
        break;
      }
    }

    drop(line_breaker);

    let mut floats_max_size = FloatsMaxSize::default();
    let mut result = LayoutUnit::ZERO;

    for (inline_size, floats) in lines {
      for float_node in floats {
        let (min_size, max_size) = atomic_inline.min_max_sizes(float_node);
        let inline_margins = float_inline_margins(float_node, direction);
        let float_style = &float_node.context.style.base_data;

        if mode == LineBreakerMode::MinContent {
          result = result.max(min_size + inline_margins);
        }

        floats_max_size.floating_objects.push(FloatingObject {
          float: float_style.float.resolve(direction),
          clear: float_style.clear.resolve(direction),
          inline_max_size_with_margin: max_size + inline_margins,
        });
      }

      result = match mode {
        LineBreakerMode::MinContent => result.max(inline_size),
        _ => floats_max_size.compute_max_size_for_line(inline_size, result),
      };
    }

    result
  }
}

/// The inline sum of `node`'s margins in a container of `direction`.
fn float_inline_margins(node: &RenderNode, direction: TextDirection) -> LayoutUnit {
  LineBoxStrut::from_physical(node.margin_px(), direction).inline_sum()
}

#[cfg(test)]
mod tests {
  use crate::layout::min_max_sizes::MinMaxSizes;
  use crate::layout::{
    inline::{
      inline_node::InlineNode,
      testing::{TestAtomicInline, render_tree, span},
    },
    node::Node,
  };

  /// Blink's `MinMaxTest`, at 6px a character where Ahem has 10px; lengths in px stay.
  fn min_max(children: impl Into<Vec<Node>>, css: &str) -> (f32, f32) {
    let block = render_tree(children, css);
    let node = InlineNode::new(&block);
    let data = node.prepare_layout();
    let MinMaxSizes { min_size, max_size } =
      node.compute_min_max_sizes(&data, &mut TestAtomicInline);

    (min_size.to_float(), max_size.to_float())
  }

  #[test]
  fn words() {
    assert_eq!(min_max([Node::text("AB CDEF")], ""), (24.0, 42.0));
  }

  /// The element boundary is in the middle of a word.
  #[test]
  fn word_across_elements() {
    assert_eq!(min_max([Node::text("A B"), span("C D")], ""), (12.0, 36.0));
  }

  #[test]
  fn pre_wrap_trailing_spaces() {
    assert_eq!(
      min_max(
        [Node::text("12345 6789 ")],
        ".container { white-space: pre-wrap }"
      ),
      (30.0, 66.0)
    );
  }

  #[test]
  fn text_indent() {
    assert_eq!(
      min_max(
        [Node::text("6 12345 12")],
        ".container { text-indent: 50px }"
      ),
      (56.0, 110.0)
    );
  }

  #[test]
  fn negative_text_indent() {
    assert_eq!(
      min_max(
        [Node::text("43210123 1234 12")],
        ".container { text-indent: -50px }"
      ),
      (24.0, 46.0)
    );
  }

  #[test]
  fn floats() {
    assert_eq!(
      min_max(
        [
          Node::text("XXX "),
          Node::container([]).with_class_name("left"),
          Node::text(" XXXX"),
        ],
        ".left { float: left; width: 50px }",
      ),
      (50.0, 98.0)
    );
  }

  /// A float that clears the one before it starts another line of floats.
  #[test]
  fn floats_with_clearance() {
    assert_eq!(
      min_max(
        [
          Node::text("XXX "),
          Node::container([]).with_class_name("left"),
          Node::container([]).with_class_name("right"),
          Node::container([]).with_class_name("left"),
          Node::text(" XXX"),
        ],
        ".left { float: left; width: 40px } .right { float: right; clear: left; width: 50px }",
      ),
      (50.0, 132.0)
    );
  }
}
