//! Where a laid-out tree puts each box and text run, for callers that measure instead of paint.

use std::collections::HashMap;

use serde::Serialize;

use crate::{
  Error, Result,
  font_style::SizedFontStyle,
  geometry::{AvailableSpace, ComputedLayout as Layout, NodeId, Size},
  layout::{
    inline::{
      InlineItem, InlineLayoutMode, InlineLayoutRequest, MeasuredInlineBox, collect_inline_items,
      create_inline_layout,
    },
    node::NodeKind,
    tree::{ContainingBlocks, LayoutResults, RenderNode},
  },
  style::{Affine, Display},
};

/// Information about a text run in an inline layout.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeasuredTextRun {
  /// The text content of this run.
  pub text: String,
  /// The x position of the run.
  pub x: f32,
  /// The y position of the run.
  pub y: f32,
  /// The width of the run.
  pub width: f32,
  /// The height of the run.
  pub height: f32,
}

/// The result of a layout measurement.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeasuredNode {
  /// The width of the node.
  pub width: f32,
  /// The height of the node.
  pub height: f32,
  /// The transform matrix of the node.
  pub transform: [f32; 6],
  /// The children of the node (including inline boxes).
  pub children: Vec<MeasuredNode>,
  /// Text runs for inline layouts.
  pub runs: Vec<MeasuredTextRun>,
}

impl MeasuredNode {
  /// Measures every node of `root`, laid out in `layout_results`, with percentages on the root
  /// resolving against `container_size`.
  pub fn of(
    root: &mut RenderNode,
    layout_results: &LayoutResults,
    container_size: Size<Option<f32>>,
  ) -> Result<Self> {
    let mut visits = vec![TraversalVisit::Enter(TraversalEnter {
      path: Vec::new(),
      node_id: NodeId::ROOT,
      transform: Affine::IDENTITY,
      container_size,
    })];
    let mut measured_by_node_id: HashMap<usize, MeasuredNode> = HashMap::new();
    let mut containing_blocks = ContainingBlocks::default();

    while let Some(visit) = visits.pop() {
      match visit {
        TraversalVisit::Enter(TraversalEnter {
          path,
          node_id,
          mut transform,
          container_size,
        }) => {
          let Some(current) = root.node_at_path_mut(&path) else {
            return Err(Error::InvalidLayoutNode(node_id.into()));
          };
          let layout = layout_results.layout(node_id)?;
          current
            .context
            .sizing
            .set_container_size(container_size.width, container_size.height);

          transform *= Affine::translation(layout.location.x, layout.location.y);
          let mut local_transform = transform;
          local_transform *= current.context.style.local_transform(
            layout.size.width,
            layout.size.height,
            &current.context.sizing,
          );
          containing_blocks.record_transform(node_id, local_transform);

          if current.should_create_inline_layout() {
            let (runs, inline_boxes) =
              measure_inline(current, collect_inline_items(current), layout);
            // Inline layout places boxes against the content box, while every measured node's
            // transform is absolute.
            let content_offset = layout.content_box_offset();
            let children = inline_boxes
              .into_iter()
              .map(|inline_box| MeasuredNode {
                width: inline_box.width,
                height: inline_box.height,
                transform: (local_transform
                  * Affine::translation(
                    inline_box.x + content_offset.x,
                    inline_box.y + content_offset.y,
                  ))
                .to_cols_array(),
                children: Vec::new(),
                runs: Vec::new(),
              })
              .collect();

            measured_by_node_id.insert(
              usize::from(node_id),
              MeasuredNode::from_layout(layout, local_transform, children, runs),
            );
            continue;
          }

          // Paint always draws a text node's own text, even when generated
          // content gave it box children; its runs sit beside those children.
          let runs = if current.context.style.display != Display::None
            && !current.has_anonymous_text_item_child()
            && let Some(text) = current.node.as_ref().and_then(|node| match &node.kind {
              NodeKind::Text(data) => Some(data.text.as_str()),
              _ => None,
            }) {
            let item = InlineItem::Text {
              text: text.into(),
              context: &current.context,
              link: None,
              decorations: None,
            };

            measure_inline(current, vec![item], layout).0
          } else {
            Vec::new()
          };

          let layout_children = if current.children.is_some() {
            layout_results.box_children(node_id)?
          } else {
            &[]
          };

          if layout_children.is_empty() {
            measured_by_node_id.insert(
              usize::from(node_id),
              MeasuredNode::from_layout(layout, local_transform, Vec::new(), runs),
            );
            continue;
          }

          let child_container_size = Size {
            width: Some(layout.content_box_width()),
            height: Some(layout.content_box_height()),
          };
          containing_blocks.record_content_box(node_id, child_container_size);

          visits.push(TraversalVisit::Exit(MeasureExit {
            node_id,
            layout,
            local_transform,
            runs,
            child_ids: layout_children.iter().map(|child| child.node_id).collect(),
          }));

          for child in layout_children.iter().rev() {
            let mut child_path = path.clone();
            child_path.push(child.render_index);
            let (base_transform, base_container) =
              containing_blocks.base_for(child, local_transform, child_container_size);
            visits.push(TraversalVisit::Enter(TraversalEnter {
              path: child_path,
              node_id: child.node_id,
              transform: base_transform,
              container_size: base_container,
            }));
          }
        }
        TraversalVisit::Exit(MeasureExit {
          node_id,
          layout,
          local_transform,
          runs,
          child_ids,
        }) => {
          let mut children = Vec::with_capacity(child_ids.len());
          for child_id in child_ids {
            let Some(child) = measured_by_node_id.remove(&usize::from(child_id)) else {
              return Err(Error::InvalidLayoutNode(child_id.into()));
            };
            children.push(child);
          }

          measured_by_node_id.insert(
            usize::from(node_id),
            MeasuredNode::from_layout(layout, local_transform, children, runs),
          );
        }
      };
    }

    measured_by_node_id
      .remove(&usize::from(NodeId::ROOT))
      .ok_or(Error::InvalidLayoutNode(NodeId::ROOT.into()))
  }

  fn from_layout(
    layout: Layout,
    local_transform: Affine,
    children: Vec<MeasuredNode>,
    runs: Vec<MeasuredTextRun>,
  ) -> Self {
    Self {
      width: layout.size.width,
      height: layout.size.height,
      transform: local_transform.to_cols_array(),
      children,
      runs,
    }
  }
}

struct TraversalEnter {
  path: Vec<usize>,
  node_id: NodeId,
  transform: Affine,
  container_size: Size<Option<f32>>,
}

enum TraversalVisit {
  Enter(TraversalEnter),
  Exit(MeasureExit),
}

struct MeasureExit {
  node_id: NodeId,
  layout: Layout,
  local_transform: Affine,
  runs: Vec<MeasuredTextRun>,
  child_ids: Vec<NodeId>,
}

/// Lays `items` out in `node`'s content box and reads back the text runs and
/// inline boxes it produced.
fn measure_inline(
  node: &RenderNode,
  items: Vec<InlineItem<'_>>,
  layout: Layout,
) -> (Vec<MeasuredTextRun>, Vec<MeasuredInlineBox>) {
  let font_style = SizedFontStyle::from_style(&node.context.style, &node.context);
  let built = create_inline_layout(InlineLayoutRequest::in_available_space(
    items,
    Size {
      width: AvailableSpace::Definite(layout.unsnapped_content.width),
      height: AvailableSpace::Definite(layout.unsnapped_content.height),
    },
    Size::NONE,
    &font_style,
    &node.context,
    InlineLayoutMode::Measure,
  ));
  let (runs, inline_boxes) = built.measure_runs(layout);
  let runs = runs
    .into_iter()
    .map(|run| MeasuredTextRun {
      text: run.text.to_string(),
      x: run.x,
      y: run.y,
      width: run.width,
      height: run.height,
    })
    .collect();

  (runs, inline_boxes)
}
