//! How far above its page a box can paint, bounded from the render tree before any page is laid out.

use crate::{
  context::RenderContext,
  font_style::SizedFontStyle,
  geometry::Size,
  layout::{
    layout_style::{LayoutLength, LayoutStyle},
    out_of_flow::depends_on_containing_block_block_size,
    tree::RenderNode,
  },
  scene::{box_ink_reach, filter_reach, has_inline_paint_content, text_ink_reach},
  style::{Display, Position},
};

/// How many font sizes above its line a glyph's ink is taken to reach, which a font would have to
/// exceed to paint a line past it.
const GLYPH_REACH_EMS: f32 = 4.0;

/// The container size a probe resolves container-relative lengths against, which differs between
/// the two probes so a length that depends on it shows.
const PROBE_SIZES: [f32; 2] = [0.0, 1024.0];

/// An upper bound on how many px above the page layout puts it on a box under `root` can paint,
/// or none when the layout decides: a transform, a percentage offset, a box held back to its
/// containing block's last fragment, or a length that depends on the size of its container.
///
/// A fragment on page `p` paints no higher than `p` pages down less this bound. A box sits at or
/// below its page's top but for the negative margins before it and the offsets of the boxes around
/// it, and what it paints reaches past it by its shadow, outline, glyph overshoot and the filters
/// that spread it. The bound adds every negative margin of the tree, which pull the boxes after
/// them up, to the largest sum of offsets and filter reaches down a chain of boxes and the ink of
/// the box at its end, plus a px for bounds rounded outwards.
///
/// Glyph ink is taken to reach at most [`GLYPH_REACH_EMS`] font sizes above its line, which is an
/// assumption about fonts and not a measurement.
pub fn reach_back(root: &RenderNode) -> Option<f32> {
  let mut margins = 0.0_f32;
  let mut chain = 0.0_f32;
  let mut stack = vec![(root, 0.0_f32)];

  while let Some((node, offset)) = stack.pop() {
    let probes = Probes::of(node);
    let styles = probes.styles(node);

    if node.context.style.has_transform_related_property() {
      return None;
    }

    margins += negative_margins(&styles)?;

    let offset = offset + displacement(&styles)? + probes.filter()?;

    chain = chain.max(offset + probes.ink(node, &styles)?);
    stack.extend(node.children.iter().flatten().map(|child| (child, offset)));
  }
  Some(margins + chain + 1.0)
}

/// A box's context resolved twice, against different container sizes.
struct Probes([RenderContext; 2]);

impl Probes {
  fn of(node: &RenderNode) -> Self {
    Self(PROBE_SIZES.map(|size| {
      let mut context = node.context.clone();

      context.sizing.set_container_size(Some(size), Some(size));
      context
    }))
  }

  fn styles(&self, node: &RenderNode) -> [LayoutStyle; 2] {
    [0, 1].map(|probe| node.layout_box_style(&self.0[probe].sizing))
  }

  /// How far the box's filters spread what it and the boxes inside it paint.
  fn filter(&self) -> Option<f32> {
    same(filter_reach(&self.0[0]), filter_reach(&self.0[1]))
  }

  /// How far the box's own ink reaches past its border box, none when it depends on the container.
  fn ink(&self, node: &RenderNode, styles: &[LayoutStyle; 2]) -> Option<f32> {
    let [first, second] = &self.0;
    let sizes = PROBE_SIZES.map(|size| Size {
      width: size,
      height: size,
    });
    let mut ink = same(
      box_ink_reach(first, sizes[0]),
      box_ink_reach(second, sizes[1]),
    )?;

    if styles[0].display == Display::Inline {
      let edge = |style: &LayoutStyle| {
        let padding = &style.padding;

        [padding.top, padding.right, padding.bottom, padding.left]
          .into_iter()
          .map(|length| match length {
            LayoutLength::Auto => Some(0.0),
            LayoutLength::Fixed(px) => Some(px.max(0.0)),
            _ => None,
          })
          .chain(
            [
              style.border.top,
              style.border.right,
              style.border.bottom,
              style.border.left,
            ]
            .map(Some),
          )
          .try_fold(0.0_f32, |edge, side| Some(edge.max(side?)))
      };

      ink += same(edge(&styles[0])?, edge(&styles[1])?)?;
    }
    if has_inline_paint_content(node) {
      let text = |context: &RenderContext| {
        let font_style = SizedFontStyle::from_style(&context.style, context);

        GLYPH_REACH_EMS * font_style.sizing.font_size + text_ink_reach(&font_style)
      };

      ink += same(text(first), text(second))?;
    }
    Some(ink)
  }
}

/// `value` when both probes resolve it alike.
fn same(first: f32, second: f32) -> Option<f32> {
  (first == second).then_some(first)
}

/// `length` in px when both probes resolve it to the same fixed length or `auto`, as zero.
fn fixed(styles: &[LayoutStyle; 2], length: impl Fn(&LayoutStyle) -> LayoutLength) -> Option<f32> {
  match (length(&styles[0]), length(&styles[1])) {
    (LayoutLength::Auto, LayoutLength::Auto) => Some(0.0),
    (LayoutLength::Fixed(first), LayoutLength::Fixed(second)) => same(first, second),
    _ => None,
  }
}

/// How far the box's margins that are negative pull the boxes after them up.
fn negative_margins(styles: &[LayoutStyle; 2]) -> Option<f32> {
  let sides = [
    |style: &LayoutStyle| style.margin.top,
    |style: &LayoutStyle| style.margin.right,
    |style: &LayoutStyle| style.margin.bottom,
    |style: &LayoutStyle| style.margin.left,
  ];
  let mut pulled = 0.0;

  for side in sides {
    // A percentage of a positive width is never negative.
    pulled += match (side(&styles[0]), side(&styles[1])) {
      (LayoutLength::Percent(first), LayoutLength::Percent(second))
        if first >= 0.0 && second >= 0.0 =>
      {
        0.0
      }
      _ => (-fixed(styles, side)?).max(0.0),
    };
  }
  Some(pulled)
}

/// How far the box's own offset moves it up from where the flow puts it.
fn displacement(styles: &[LayoutStyle; 2]) -> Option<f32> {
  let style = &styles[0];

  match style.position {
    Position::Static => Some(0.0),
    Position::Relative => Some(
      fixed(styles, |style| style.inset.top)?.abs()
        + fixed(styles, |style| style.inset.bottom)?.abs(),
    ),
    Position::Absolute | Position::Fixed => {
      if depends_on_containing_block_block_size(style) {
        return None;
      }

      Some((-fixed(styles, |style| style.inset.top)?).max(0.0))
    }
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::reach_back;
  use crate::{
    context::RenderContext,
    layout::{node::Node, tree::RenderNode},
    resources::font::Fonts,
    style::{SizingContext, StyleSheet},
    viewport::Viewport,
  };

  /// The reach back of a block with a child styled by `css`, among blocks without any.
  fn reach_of(css: &str) -> Option<f32> {
    let stylesheet = StyleSheet::parse(&format!(
      ".block {{ display: block; height: 50px }} .child {{ {css} }}"
    ))
    .expect("stylesheet parses");
    let context = RenderContext::builder()
      .fonts(Fonts::default().snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::default())
          .build(),
      )
      .stylesheet(Arc::new(stylesheet))
      .build();
    let root = Node::container([
      Node::container(Vec::<Node>::new()).with_class_name("block"),
      Node::container(Vec::<Node>::new()).with_class_name("block child"),
    ])
    .with_class_name("block");

    reach_back(&RenderNode::from_node(&context, root))
  }

  #[test]
  fn plain_boxes_reach_back_by_the_rounding_of_their_bounds() {
    assert_eq!(reach_of(""), Some(1.0));
  }

  #[test]
  fn shadows_offsets_and_negative_margins_add_their_reach() {
    assert_eq!(reach_of("box-shadow: 0 0 0 30px red"), Some(31.0));
    assert_eq!(reach_of("position: relative; top: -40px"), Some(41.0));
    assert_eq!(reach_of("position: absolute; top: -25px"), Some(26.0));
    assert_eq!(reach_of("margin-top: -60px"), Some(61.0));
    assert_eq!(reach_of("margin-top: 60px"), Some(1.0));
  }

  #[test]
  fn boxes_the_layout_places_anywhere_have_no_bound() {
    for css in [
      "transform: translateY(-300px)",
      "position: relative; top: 10%",
      "position: absolute; bottom: 0",
      "position: absolute; top: 10%",
      "box-shadow: 0 10% 0 0 red",
      "margin-top: -10%",
    ] {
      assert_eq!(reach_of(css), None, "{css}");
    }
  }
}
