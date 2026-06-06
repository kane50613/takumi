use std::{collections::HashMap, ops::Range, sync::Arc};

use image::RgbaImage;
use parley::{GlyphRun, InlineBoxKind, PositionedLayoutItem};
use serde::Serialize;
use taffy::{AvailableSpace, Layout, NodeId, TaffyError, geometry::Size};
use typed_builder::TypedBuilder;

use crate::{
  Error, GlobalContext, Result,
  layout::{
    Viewport,
    inline::{
      InlineBrush, InlineLayoutMode, InlineLayoutRequest, ProcessedInlineSpan,
      collect_inline_items, create_inline_constraint, create_inline_layout,
      get_parent_font_metrics, resolve_inline_line_metrics, resolve_inline_line_states,
      resolve_visual_inline_box, text_fit_line_alignment_correction,
    },
    node::Node,
    style::{Affine, StyleSheet},
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  rendering::{
    AnimationFrame, Canvas, DitheringAlgorithm, RenderContext, apply_dithering,
    get_node_mut_by_path, scale_text_fit_x,
    stacking_context::{
      apply_transform, build_stacking_contexts, collect_layout_children, paint_context,
    },
    to_sized_font_style,
  },
  resources::image::ImageSource,
};

#[derive(Clone, TypedBuilder)]
/// Options for rendering a node. Construct using [`RenderOptions::builder`] to avoid breaking changes.
pub struct RenderOptions<'g> {
  /// The viewport to render the node in.
  pub(crate) viewport: Viewport,
  /// The global context.
  pub(crate) global: &'g GlobalContext,
  /// The node to render.
  pub(crate) node: Node,
  /// Whether to draw debug borders.
  #[builder(default = false)]
  pub(crate) draw_debug_border: bool,
  /// The resources fetched externally.
  #[builder(default)]
  pub(crate) fetched_resources: HashMap<Arc<str>, ImageSource>,
  /// CSS stylesheets to apply before layout/rendering.
  #[builder(default)]
  pub(crate) stylesheet: StyleSheet,
  /// Global animation time in milliseconds.
  #[builder(default = 0)]
  pub(crate) time_ms: u64,
  /// Output dithering algorithm. Only used by encoding frontends.
  #[builder(default)]
  pub(crate) dithering: DitheringAlgorithm,
}

impl<'g> RenderOptions<'g> {
  /// Returns a reference to the viewport.
  pub fn viewport(&self) -> &Viewport {
    &self.viewport
  }

  /// Returns a reference to the root node.
  pub fn node(&self) -> &Node {
    &self.node
  }
}

#[derive(Clone, TypedBuilder)]
/// A single scene in a sequential animation timeline.
pub struct SequentialScene<'g> {
  /// Render options used when this scene is active.
  pub(crate) options: RenderOptions<'g>,
  /// Duration of this scene in milliseconds.
  pub(crate) duration_ms: u32,
}

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

fn measured_run_text<'a>(
  text: &'a str,
  spans: &[ProcessedInlineSpan<'_, '_>],
  glyph_run: &GlyphRun<'_, InlineBrush>,
) -> &'a str {
  let text_range = glyph_run.run().text_range();
  let Some(span_id) = glyph_run.style().brush.source_span_id else {
    return slice_text_at_char_boundaries(text, text_range);
  };

  let Some(ProcessedInlineSpan::Text { byte_range, .. }) = spans.get(span_id as usize) else {
    return slice_text_at_char_boundaries(text, text_range);
  };

  let start = text_range.start.max(byte_range.start);
  let end = text_range.end.min(byte_range.end);
  slice_text_at_char_boundaries(text, start..end)
}

fn slice_text_at_char_boundaries(text: &str, byte_range: Range<usize>) -> &str {
  if byte_range.start >= byte_range.end || byte_range.start >= text.len() {
    return "";
  }

  let end = byte_range.end.min(text.len());
  let start = text.ceil_char_boundary(byte_range.start.min(end));
  let end = text.floor_char_boundary(end);
  if start >= end {
    return "";
  }

  &text[start..end]
}

struct TraversalEnter {
  path: Vec<usize>,
  node_id: NodeId,
  transform: Affine,
  container_size: Size<Option<f32>>,
}

enum TraversalVisit<Exit> {
  Enter(TraversalEnter),
  Exit(Exit),
}

struct MeasureExit {
  node_id: NodeId,
  width: f32,
  height: f32,
  local_transform: Affine,
  runs: Vec<MeasuredTextRun>,
  child_ids: Vec<NodeId>,
}

/// Measures the layout of a node.
pub fn measure_layout<'g>(options: RenderOptions<'g>) -> Result<MeasuredNode> {
  let RenderOptions {
    viewport,
    global,
    node,
    draw_debug_border,
    fetched_resources,
    stylesheet,
    time_ms,
    dithering: _,
  } = options;
  let mut render_context = RenderContext::new(
    global,
    viewport,
    fetched_resources,
    stylesheet.into(),
    time_ms,
  );
  render_context.draw_debug_border = draw_debug_border;
  let mut root = RenderNode::from_node(&render_context, node);
  let mut tree = LayoutTree::from_render_node(&root);
  tree.compute_layout(render_context.sizing.viewport.into());
  let layout_results = tree.into_results();

  collect_measure_result(
    &mut root,
    &layout_results,
    layout_results.root_node_id(),
    Affine::IDENTITY,
    Size {
      width: viewport.size.width.map(|value| value as f32),
      height: viewport.size.height.map(|value| value as f32),
    },
  )
}

fn collect_measure_result<'g>(
  node: &mut RenderNode<'g>,
  layout_results: &LayoutResults,
  node_id: NodeId,
  transform: Affine,
  container_size: Size<Option<f32>>,
) -> Result<MeasuredNode> {
  let mut visits = vec![TraversalVisit::Enter(TraversalEnter {
    path: Vec::new(),
    node_id,
    transform,
    container_size,
  })];
  let mut measured_by_node_id: HashMap<usize, MeasuredNode> = HashMap::new();
  // Hoisted out-of-flow nodes resolve geometry against their containing block.
  // Memoize each node's transform and content box for hoisted children to base on.
  let mut node_transforms: HashMap<NodeId, Affine> = HashMap::new();
  let mut node_content_box: HashMap<NodeId, Size<Option<f32>>> = HashMap::new();

  while let Some(visit) = visits.pop() {
    match visit {
      TraversalVisit::Enter(TraversalEnter {
        path,
        node_id,
        mut transform,
        container_size,
      }) => {
        let Some(current) = get_node_mut_by_path(node, &path) else {
          return Err(Error::LayoutError(TaffyError::InvalidInputNode(node_id)));
        };
        let layout = *layout_results.layout(node_id)?;
        current.context.sizing.container_size = container_size;

        transform *= Affine::translation(layout.location.x, layout.location.y);
        let mut local_transform = transform;
        apply_transform(
          &mut local_transform,
          &current.context.style,
          layout.size,
          &current.context.sizing,
        );
        node_transforms.insert(node_id, local_transform);

        let mut children = Vec::new();
        let mut runs = Vec::new();

        if current.should_create_inline_layout() {
          let font_style = to_sized_font_style(&current.context.style, &current.context);
          let (max_width, max_height) = create_inline_constraint(
            &current.context,
            Size {
              width: AvailableSpace::Definite(layout.content_box_width()),
              height: AvailableSpace::Definite(layout.content_box_height()),
            },
            Size::NONE,
          );

          let built = create_inline_layout(InlineLayoutRequest {
            items: collect_inline_items(current),
            available_space: Size {
              width: AvailableSpace::Definite(layout.content_box_width()),
              height: AvailableSpace::Definite(layout.content_box_height()),
            },
            max_width,
            max_height,
            style: &font_style,
            global: current.context.global,
            mode: InlineLayoutMode::Measure,
          });
          let parent_font_metrics = get_parent_font_metrics(&built.layout);
          let inline_offset = taffy::Point::ZERO;
          let line_metrics = resolve_inline_line_metrics(
            &built.layout,
            &built.spans,
            parent_font_metrics,
            &built.line_scales,
          );
          let line_states = resolve_inline_line_states(
            &built.layout,
            &built.spans,
            parent_font_metrics,
            &built.line_scales,
          );
          for (line_index, line) in built.layout.lines().enumerate() {
            let baseline_shift = line_states[line_index].baseline_shift;
            let resolved_metrics = line_metrics[line_index];
            let line_scale = built.line_scales.get(line_index).copied().unwrap_or(1.0);
            let (line_scale_origin_x, line_alignment_correction) =
              text_fit_line_alignment_correction(
                &line,
                line_scale,
                layout.content_box_size().width,
              );
            let line_scale_origin = taffy::Point {
              x: line_scale_origin_x + inline_offset.x,
              y: resolved_metrics.resolved_baseline + inline_offset.y,
            };
            let mut static_inline_prefix = 0.0_f32;
            for item in line.items() {
              match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                  let text = measured_run_text(&built.text, &built.spans, &glyph_run);
                  if text.is_empty() {
                    continue;
                  }
                  let run = glyph_run.run();
                  let metrics = run.metrics();
                  let mut x = glyph_run.offset() + inline_offset.x;
                  let mut y =
                    glyph_run.baseline() + baseline_shift - metrics.ascent + inline_offset.y;
                  let mut width = glyph_run.advance();
                  let mut height = metrics.ascent + metrics.descent;
                  if (line_scale - 1.0).abs() > f32::EPSILON {
                    x = scale_text_fit_x(
                      x,
                      line_scale_origin.x,
                      line_scale,
                      static_inline_prefix,
                      line_alignment_correction,
                    );
                    y = line_scale_origin.y + (y - line_scale_origin.y) * line_scale;
                    width *= line_scale;
                    height *= line_scale;
                  }

                  runs.push(MeasuredTextRun {
                    text: text.to_string(),
                    x,
                    y,
                    width,
                    height,
                  });
                }
                PositionedLayoutItem::InlineBox(positioned_box) => {
                  if positioned_box.kind != InlineBoxKind::InFlow {
                    continue;
                  }
                  let Some(positioned_box) = resolve_visual_inline_box(
                    positioned_box,
                    Some(line_states[line_index]),
                    &built.spans,
                  ) else {
                    continue;
                  };
                  let positioned_box_x = scale_text_fit_x(
                    positioned_box.x,
                    line_scale_origin_x,
                    line_scale,
                    static_inline_prefix,
                    line_alignment_correction,
                  );
                  static_inline_prefix += positioned_box.width;

                  let inline_transform =
                    Affine::translation(positioned_box_x, positioned_box.y) * local_transform;

                  children.push(MeasuredNode {
                    width: positioned_box.width,
                    height: positioned_box.height,
                    transform: inline_transform.to_cols_array(),
                    children: Vec::new(),
                    runs: Vec::new(),
                  });
                }
              }
            }
          }

          for positioned_box in built.custom_inline_boxes {
            let inline_transform =
              Affine::translation(positioned_box.x, positioned_box.y) * local_transform;
            children.push(MeasuredNode {
              width: positioned_box.width,
              height: positioned_box.height,
              transform: inline_transform.to_cols_array(),
              children: Vec::new(),
              runs: Vec::new(),
            });
          }

          measured_by_node_id.insert(
            usize::from(node_id),
            create_measured_node(layout, local_transform, children, runs),
          );
          continue;
        }

        if current.children.is_none() {
          measured_by_node_id.insert(
            usize::from(node_id),
            create_measured_node(layout, local_transform, children, runs),
          );
          continue;
        }

        let layout_children = collect_layout_children(layout_results, node_id)?;
        if layout_children.is_empty() {
          measured_by_node_id.insert(
            usize::from(node_id),
            create_measured_node(layout, local_transform, children, runs),
          );
          continue;
        }

        let child_container_size = Size {
          width: Some(layout.content_box_width()),
          height: Some(layout.content_box_height()),
        };
        node_content_box.insert(node_id, child_container_size);

        visits.push(TraversalVisit::Exit(MeasureExit {
          node_id,
          width: layout.size.width,
          height: layout.size.height,
          local_transform,
          runs,
          child_ids: layout_children.iter().map(|child| child.node_id).collect(),
        }));

        for child in layout_children.iter().rev() {
          let mut child_path = path.clone();
          child_path.push(child.render_index);
          let (base_transform, base_container) = match child.hoisted_cb {
            Some(cb) => (
              *node_transforms.get(&cb).unwrap_or(&local_transform),
              *node_content_box.get(&cb).unwrap_or(&child_container_size),
            ),
            None => (local_transform, child_container_size),
          };
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
        width,
        height,
        local_transform,
        runs,
        child_ids,
      }) => {
        let mut children = Vec::with_capacity(child_ids.len());
        for child_id in child_ids {
          let Some(child) = measured_by_node_id.remove(&usize::from(child_id)) else {
            return Err(Error::LayoutError(TaffyError::InvalidInputNode(child_id)));
          };
          children.push(child);
        }

        measured_by_node_id.insert(
          usize::from(node_id),
          MeasuredNode {
            width,
            height,
            transform: local_transform.to_cols_array(),
            children,
            runs,
          },
        );
      }
    };
  }

  measured_by_node_id
    .remove(&usize::from(node_id))
    .ok_or_else(|| Error::LayoutError(TaffyError::InvalidInputNode(node_id)))
}

fn create_measured_node(
  layout: Layout,
  local_transform: Affine,
  children: Vec<MeasuredNode>,
  runs: Vec<MeasuredTextRun>,
) -> MeasuredNode {
  MeasuredNode {
    width: layout.size.width,
    height: layout.size.height,
    transform: local_transform.to_cols_array(),
    children,
    runs,
  }
}

/// Renders a node to an image.
pub fn render<'g>(options: RenderOptions<'g>) -> Result<RgbaImage> {
  let RenderOptions {
    viewport,
    global,
    node,
    draw_debug_border,
    fetched_resources,
    stylesheet,
    time_ms,
    dithering,
  } = options;

  let mut render_context = RenderContext::new(
    global,
    viewport,
    fetched_resources,
    stylesheet.into(),
    time_ms,
  );
  render_context.draw_debug_border = draw_debug_border;

  let mut root = RenderNode::from_node(&render_context, node);
  let mut tree = LayoutTree::from_render_node(&root);
  tree.compute_layout(render_context.sizing.viewport.into());
  let layout_results = tree.into_results();
  let root_node_id = layout_results.root_node_id();
  let root_size = layout_results
    .layout(root_node_id)?
    .size
    .map(|size| size.round() as u32);

  let root_size = root_size.zip_map(viewport.into(), |size, viewport| {
    if let AvailableSpace::Definite(defined) = viewport {
      defined as u32
    } else {
      size
    }
  });

  if root_size.width == 0 || root_size.height == 0 {
    return Err(Error::InvalidViewport);
  }

  let mut canvas = Canvas::new(root_size);

  render_node(
    &mut root,
    &layout_results,
    root_node_id,
    &mut canvas,
    Affine::IDENTITY,
    Size {
      width: viewport.size.width.map(|value| value as f32),
      height: viewport.size.height.map(|value| value as f32),
    },
  )?;

  let mut image = canvas.into_inner()?;
  apply_dithering(&mut image, dithering);

  Ok(image)
}

/// Renders a node at a specific time on the global animation timeline.
pub fn render_at_time<'g>(mut options: RenderOptions<'g>, time_ms: u64) -> Result<RgbaImage> {
  options.time_ms = time_ms;
  render(options)
}

/// Renders the active scene for a sequential animation timeline at `time_ms`.
pub fn render_sequence_at_time<'g>(
  scenes: &[SequentialScene<'g>],
  time_ms: u64,
) -> Result<RgbaImage> {
  let Some((scene, local_time_ms)) = resolve_scene_at_time(scenes, time_ms) else {
    return Err(Error::InvalidViewport);
  };

  render_at_time(scene.options.clone(), local_time_ms)
}

/// Renders all frames for a sequential animation timeline at a fixed frame rate.
pub fn render_sequence_animation<'g>(
  scenes: &[SequentialScene<'g>],
  fps: u32,
) -> Result<Vec<AnimationFrame>> {
  if scenes.is_empty() || fps == 0 {
    return Ok(Vec::new());
  }

  let total_duration_ms = total_sequence_duration(scenes);
  if total_duration_ms == 0 {
    return Ok(Vec::new());
  }

  let frame_count = total_duration_ms
    .saturating_mul(u64::from(fps))
    .div_ceil(1000);
  let mut frames = Vec::with_capacity(frame_count as usize);

  for frame_index in 0..frame_count {
    let start_ms = frame_index * 1000 / u64::from(fps);
    let end_ms = ((frame_index + 1) * 1000 / u64::from(fps)).min(total_duration_ms);
    let frame_duration_ms = end_ms.saturating_sub(start_ms);
    if frame_duration_ms == 0 {
      continue;
    }

    let image = render_sequence_at_time(scenes, start_ms)?;
    frames.push(AnimationFrame::new(image, frame_duration_ms as u32));
  }

  Ok(frames)
}

fn total_sequence_duration<'g>(scenes: &[SequentialScene<'g>]) -> u64 {
  scenes
    .iter()
    .map(|scene| u64::from(scene.duration_ms))
    .sum::<u64>()
}

fn resolve_scene_at_time<'a, 'g>(
  scenes: &'a [SequentialScene<'g>],
  time_ms: u64,
) -> Option<(&'a SequentialScene<'g>, u64)> {
  if scenes.is_empty() {
    return None;
  }

  let mut elapsed_ms = 0_u64;
  let clamped_time_ms = time_ms.min(total_sequence_duration(scenes).saturating_sub(1));

  for scene in scenes {
    let next_elapsed_ms = elapsed_ms + u64::from(scene.duration_ms);
    if clamped_time_ms < next_elapsed_ms {
      return Some((scene, clamped_time_ms - elapsed_ms));
    }
    elapsed_ms = next_elapsed_ms;
  }

  scenes
    .last()
    .map(|scene| (scene, u64::from(scene.duration_ms.saturating_sub(1))))
}

pub(crate) fn render_node<'g>(
  node: &mut RenderNode<'g>,
  layout_results: &LayoutResults,
  node_id: NodeId,
  canvas: &mut Canvas,
  transform: Affine,
  container_size: Size<Option<f32>>,
) -> Result<()> {
  let contexts = build_stacking_contexts(node, layout_results, node_id, transform, container_size)?;
  paint_context(node, &contexts, layout_results, canvas, 0)
}

#[cfg(test)]
mod tests {
  use image::Rgba;

  use super::{
    RenderOptions, SequentialScene, render, render_sequence_animation, resolve_scene_at_time,
    slice_text_at_char_boundaries,
  };
  use crate::{
    GlobalContext,
    layout::{
      Viewport,
      node::Node,
      style::{
        AnimationFillMode, AnimationTime, AnimationTimingFunction, Color, ColorInput, Display,
        KeyframeRule, KeyframesRule, Length, Length::Px, Position, Style, StyleDeclaration,
      },
    },
    rendering::measure_layout,
  };

  fn make_scene<'g>(global: &'g GlobalContext, duration_ms: u32) -> SequentialScene<'g> {
    let options = RenderOptions::builder()
      .global(global)
      .viewport(Viewport::new((10, 10)))
      .node(Node::container([]))
      .build();

    SequentialScene::builder()
      .duration_ms(duration_ms)
      .options(options)
      .build()
  }

  #[test]
  fn resolve_scene_at_time_uses_cumulative_durations() {
    let global = GlobalContext::default();
    let scenes = vec![make_scene(&global, 100), make_scene(&global, 200)];

    let scene = resolve_scene_at_time(&scenes, 50);
    assert!(scene.is_some());
    let local_time = scene.map_or(0, |(_, local_time)| local_time);
    assert_eq!(local_time, 50);

    let scene = resolve_scene_at_time(&scenes, 150);
    assert!(scene.is_some());
    let local_time = scene.map_or(0, |(_, local_time)| local_time);
    assert_eq!(local_time, 50);
  }

  #[test]
  fn resolve_scene_at_time_clamps_to_last_scene() {
    let global = GlobalContext::default();
    let scenes = vec![make_scene(&global, 100), make_scene(&global, 200)];

    let scene = resolve_scene_at_time(&scenes, 500);
    assert!(scene.is_some());
    let local_time = scene.map_or(0, |(_, local_time)| local_time);
    assert_eq!(local_time, 199);
  }

  #[test]
  fn render_sequence_animation_returns_no_frames_for_zero_duration_timelines() {
    let global = GlobalContext::default();
    let scenes = vec![make_scene(&global, 0)];

    let frames_result = render_sequence_animation(&scenes, 30);
    assert!(frames_result.is_ok());
    let frames = frames_result.unwrap_or_default();

    assert!(frames.is_empty());
  }

  #[test]
  fn render_sequence_animation_uses_per_frame_integer_durations() {
    let global = GlobalContext::default();
    let scenes = vec![make_scene(&global, 150)];

    let frames_result = render_sequence_animation(&scenes, 30);
    assert!(frames_result.is_ok());
    let frames = frames_result.unwrap_or_default();
    let durations = frames
      .iter()
      .map(|frame| frame.duration_ms)
      .collect::<Vec<_>>();

    assert_eq!(durations, vec![33, 33, 34, 33, 17]);
    assert_eq!(
      durations
        .iter()
        .map(|duration| u64::from(*duration))
        .sum::<u64>(),
      150
    );
  }

  #[test]
  fn slice_text_at_char_boundaries_trims_invalid_utf8_edges() {
    let text = "a🦀b";

    assert_eq!(slice_text_at_char_boundaries(text, 0..3), "a");
    assert_eq!(slice_text_at_char_boundaries(text, 1..5), "🦀");
    assert_eq!(slice_text_at_char_boundaries(text, 2..5), "");
    assert_eq!(slice_text_at_char_boundaries(text, 0..text.len()), text);
  }

  #[test]
  fn measure_layout_supports_structured_keyframes() {
    let global = GlobalContext::default();
    let node = Node::container([]).with_tag_name("div").with_style(
      Style::default()
        .with(StyleDeclaration::width(Px(100.0)))
        .with(StyleDeclaration::animation_name(
          [Some("grow".to_string())].into(),
        ))
        .with(StyleDeclaration::animation_duration(
          [AnimationTime::from_milliseconds(1000.0)].into(),
        ))
        .with(StyleDeclaration::animation_timing_function(
          [AnimationTimingFunction::Linear].into(),
        ))
        .with(StyleDeclaration::animation_fill_mode(
          [AnimationFillMode::Both].into(),
        )),
    );

    let options = RenderOptions::builder()
      .global(&global)
      .viewport(Viewport::new((200, 100)))
      .node(node)
      .stylesheet(
        vec![KeyframesRule {
          name: "grow".to_string(),
          keyframes: vec![
            KeyframeRule::builder()
              .offsets([0.0])
              .declarations(
                Style::default()
                  .with(StyleDeclaration::width(Px(100.0)))
                  .into(),
              )
              .build(),
            KeyframeRule::builder()
              .offsets([1.0])
              .declarations(
                Style::default()
                  .with(StyleDeclaration::width(Px(200.0)))
                  .into(),
              )
              .build(),
          ],
          media_queries: Vec::new(),
        }]
        .into(),
      )
      .time_ms(500)
      .build();

    let layout_result = measure_layout(options);
    assert!(layout_result.is_ok());
    let layout = match layout_result {
      Ok(layout) => layout,
      Err(_) => return,
    };

    assert_eq!(layout.width, 150.0);
  }

  #[test]
  fn measure_resolves_absolute_against_relative_skipping_static() {
    // root(relative) > mid(static, offset by margin) > abs(absolute).
    // The absolute's containing block is the relative root, not the static
    // middle, so its transform must resolve against the root's origin (0, 0)
    // plus its own insets — independent of the static middle's offset.
    let global = GlobalContext::default();
    let abs = Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::position(Position::Absolute))
        .with(StyleDeclaration::left(Px(40.0)))
        .with(StyleDeclaration::top(Px(30.0)))
        .with(StyleDeclaration::width(Px(10.0)))
        .with(StyleDeclaration::height(Px(10.0))),
    );
    let mid = Node::container([abs]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::position(Position::Static))
        .with(StyleDeclaration::margin_left(Px(50.0)))
        .with(StyleDeclaration::margin_top(Px(50.0)))
        .with(StyleDeclaration::width(Px(100.0)))
        .with(StyleDeclaration::height(Px(100.0))),
    );
    let root = Node::container([mid]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::position(Position::Relative))
        .with(StyleDeclaration::width(Px(200.0)))
        .with(StyleDeclaration::height(Px(200.0))),
    );

    let options = RenderOptions::builder()
      .global(&global)
      .viewport(Viewport::new((200, 200)))
      .node(root)
      .build();

    let layout = match measure_layout(options) {
      Ok(layout) => layout,
      Err(_) => return,
    };
    let mid_node = &layout.children[0];
    let abs_node = &mid_node.children[0];

    // mid (static, in-flow) carries the margin offset; abs (absolute) resolves
    // against the relative root, so it sits at its own insets, not mid's offset.
    assert_eq!((mid_node.transform[4], mid_node.transform[5]), (50.0, 50.0));
    assert_eq!((abs_node.transform[4], abs_node.transform[5]), (40.0, 30.0));
  }

  #[test]
  fn absolute_positioned_children_paint_over_in_flow_background() {
    // CSS 2.1 paint order requires positioned descendants with z-index:auto/0
    // to paint above in-flow non-positioned descendants in the same stacking context.
    // Ref: https://www.w3.org/TR/CSS22/zindex.html#painting-order
    let node = Node::container([Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::position(Position::Absolute))
        .with(StyleDeclaration::left(Length::Px(0.0)))
        .with(StyleDeclaration::top(Length::Px(0.0)))
        .with(StyleDeclaration::width(Length::Px(128.0)))
        .with(StyleDeclaration::height(Length::Px(128.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::from_rgb(0xff0000),
        ))),
    )])
    .with_style(
      Style::default()
        .with(StyleDeclaration::position(Position::Relative))
        .with(StyleDeclaration::width(Length::Px(256.0)))
        .with(StyleDeclaration::height(Length::Px(256.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::from_rgb(0x0b1020),
        ))),
    );
    let global = GlobalContext::default();
    let options = RenderOptions::builder()
      .global(&global)
      .viewport(Viewport::new((256, 256)))
      .node(node.clone())
      .build();
    let measured = match measure_layout(options.clone()) {
      Ok(measured) => measured,
      Err(_) => return,
    };
    assert_eq!(measured.children.len(), 1);
    assert_eq!(measured.children[0].width, 128.0);
    assert_eq!(measured.children[0].height, 128.0);

    let rendered = match render(options) {
      Ok(rendered) => rendered,
      Err(_) => return,
    };

    let top_left = rendered.get_pixel(10, 10);
    let bottom_right = rendered.get_pixel(220, 220);

    assert_eq!(top_left, &Rgba([255, 0, 0, 255]));
    assert_eq!(bottom_right, &Rgba([11, 16, 32, 255]));
  }
}
