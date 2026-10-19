//! Where a laid-out tree puts each box and text run, for callers that measure instead of paint.

use std::collections::HashMap;

use serde::Serialize;

use crate::{
  Error, Result,
  font_style::SizedFontStyle,
  geometry::{ComputedLayout as Layout, NodeId, Size},
  layout::tree::{ContainingBlocks, LayoutResults, RenderNode},
  painter::OwnContent,
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
          containing_blocks.record_placement(node_id, local_transform);

          let own_content = OwnContent::of(current);
          let font_style = SizedFontStyle::from_style(&current.context.style, &current.context);
          let items = own_content.fragment_items(&font_style, layout).filter(|_| {
            current.should_create_inline_layout()
              || current.context.style.base_data.display != Display::None
          });
          let (runs, leading) = match items {
            Some(items) => {
              let (runs, inline_boxes) = items.measure_runs(current);
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

              (runs, children)
            }
            None => (Vec::new(), Vec::new()),
          };

          let layout_children = if current.children.is_some() {
            layout_results.box_children(node_id)?
          } else {
            &[]
          };

          if layout_children.is_empty() {
            measured_by_node_id.insert(
              usize::from(node_id),
              MeasuredNode::from_layout(layout, local_transform, leading, runs),
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
            leading,
            child_ids: layout_children.iter().map(|child| child.node_id).collect(),
          }));

          for child in layout_children.iter().rev() {
            let mut child_path = path.clone();
            child.extend_path(&mut child_path);
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
          leading,
          child_ids,
        }) => {
          let mut children = leading;

          children.reserve(child_ids.len());
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
  /// The inline boxes an inline formatting context measured, ahead of its layout children.
  leading: Vec<MeasuredNode>,
  child_ids: Vec<NodeId>,
}
