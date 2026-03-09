use std::{collections::HashMap, mem::replace, sync::Arc};

use derive_builder::Builder;
use image::RgbaImage;
use parley::PositionedLayoutItem;
use serde::Serialize;
use taffy::{AvailableSpace, Layout, NodeId, TaffyError, geometry::Size};

#[cfg(feature = "css_stylesheet_parsing")]
use crate::layout::style::selector::StyleSheet;
use crate::{
  Error, GlobalContext, Result,
  layout::{
    Viewport,
    inline::{
      InlineLayoutStage, ProcessedInlineSpan, collect_inline_items, create_inline_constraint,
      create_inline_layout,
    },
    node::Node,
    style::{
      Affine, ComputedStyle, Filter, ImageScalingAlgorithm, KeyframesRule, SpacePair,
      apply_backdrop_filter, apply_filters,
    },
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  rendering::{
    AnimationFrame, BorderProperties, Canvas, CanvasConstrain, CanvasConstrainResult,
    DitheringAlgorithm, RenderContext, RenderTime, Sizing, apply_dithering, draw_debug_border,
    inline_drawing::get_parent_x_height, overlay_image,
  },
  resources::image::ImageSource,
};

#[derive(Clone, Builder)]
#[builder(pattern = "owned")]
/// Options for rendering a node. Construct using [`RenderOptionsBuilder`] to avoid breaking changes.
pub struct RenderOptions<'g, N: Node<N>> {
  /// The viewport to render the node in.
  pub(crate) viewport: Viewport,
  /// The global context.
  pub(crate) global: &'g GlobalContext,
  /// The node to render.
  pub(crate) node: N,
  /// Whether to draw debug borders.
  #[builder(default)]
  pub(crate) draw_debug_border: bool,
  /// The resources fetched externally.
  #[builder(default)]
  pub(crate) fetched_resources: HashMap<Arc<str>, Arc<ImageSource>>,
  /// CSS stylesheets to apply before layout/rendering.
  #[builder(default)]
  pub(crate) stylesheets: Vec<String>,
  /// Structured keyframes to register alongside stylesheets.
  #[builder(default)]
  pub(crate) keyframes: Vec<KeyframesRule>,
  /// Global animation time in milliseconds.
  #[builder(default)]
  pub(crate) time_ms: u64,
  /// Output dithering algorithm. Only used by encoding frontends.
  #[builder(default)]
  pub(crate) dithering: DitheringAlgorithm,
}

#[derive(Clone, Builder)]
#[builder(pattern = "owned")]
/// A single scene in a sequential animation timeline.
pub struct SequentialScene<'g, N: Node<N>> {
  /// Render options used when this scene is active.
  pub(crate) options: RenderOptions<'g, N>,
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

struct RenderExit {
  path: Vec<usize>,
  has_constrain: bool,
  original_canvas_image: Option<RgbaImage>,
}

/// Measures the layout of a node.
pub fn measure_layout<'g, N: Node<N>>(options: RenderOptions<'g, N>) -> Result<MeasuredNode> {
  let RenderOptions {
    viewport,
    global,
    node,
    draw_debug_border,
    fetched_resources,
    stylesheets,
    keyframes,
    time_ms,
    dithering: _,
  } = options;
  #[cfg(feature = "css_stylesheet_parsing")]
  let parsed_stylesheets = build_stylesheets(stylesheets, keyframes);
  #[cfg(feature = "css_stylesheet_parsing")]
  let mut render_context = RenderContext::new(
    global,
    viewport,
    fetched_resources,
    parsed_stylesheets,
    RenderTime { time_ms },
  );
  #[cfg(not(feature = "css_stylesheet_parsing"))]
  let mut render_context =
    RenderContext::new(global, viewport, fetched_resources, RenderTime { time_ms });
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
      width: viewport.width.map(|value| value as f32),
      height: viewport.height.map(|value| value as f32),
    },
  )
}

fn collect_measure_result<'g, Nodes: Node<Nodes>>(
  node: &mut RenderNode<'g, Nodes>,
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

  while let Some(visit) = visits.pop() {
    match visit {
      TraversalVisit::Enter(TraversalEnter {
        path,
        node_id,
        mut transform,
        container_size,
      }) => {
        let Some(current) = get_node_mut_by_path(node, &path) else {
          unreachable!()
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

        let mut children = Vec::new();
        let mut runs = Vec::new();

        if current.should_create_inline_layout() {
          let font_style = current.context.style.to_sized_font_style(&current.context);
          let parent_x_height = get_parent_x_height(&current.context, &font_style);
          let (max_width, max_height) = create_inline_constraint(
            &current.context,
            Size {
              width: AvailableSpace::Definite(layout.content_box_width()),
              height: AvailableSpace::Definite(layout.content_box_height()),
            },
            Size::NONE,
          );

          let (inline_layout, text, spans) = create_inline_layout(
            collect_inline_items(current).into_iter(),
            Size {
              width: AvailableSpace::Definite(layout.content_box_width()),
              height: AvailableSpace::Definite(layout.content_box_height()),
            },
            max_width,
            max_height,
            &font_style,
            current.context.global,
            InlineLayoutStage::Measure,
          );
          let inline_offset = taffy::Point::ZERO;

          for line in inline_layout.lines() {
            for item in line.items() {
              match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                  let text_range = glyph_run.run().text_range();
                  let text = &text[text_range];
                  let run = glyph_run.run();
                  let metrics = run.metrics();

                  runs.push(MeasuredTextRun {
                    text: text.to_string(),
                    x: glyph_run.offset() + inline_offset.x,
                    y: glyph_run.baseline() - metrics.ascent + inline_offset.y,
                    width: glyph_run.advance(),
                    height: metrics.ascent + metrics.descent,
                  });
                }
                PositionedLayoutItem::InlineBox(mut positioned_box) => {
                  let item_index = positioned_box.id as usize;
                  if let Some(ProcessedInlineSpan::Box(item)) = spans.get(item_index) {
                    item.vertical_align.apply(
                      &mut positioned_box.y,
                      line.metrics(),
                      positioned_box.height,
                      parent_x_height,
                    );
                  }
                  positioned_box.x += inline_offset.x;
                  positioned_box.y += inline_offset.y;

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
              }
            }
          }

          measured_by_node_id.insert(
            usize::from(node_id),
            create_measured_node(layout, local_transform, children, runs),
          );
          continue;
        }

        let Some(render_children) = current.children.as_deref() else {
          measured_by_node_id.insert(
            usize::from(node_id),
            create_measured_node(layout, local_transform, children, runs),
          );
          continue;
        };

        let child_ids = collect_child_node_ids(layout_results, node_id, render_children.len())?;
        if child_ids.is_empty() {
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

        visits.push(TraversalVisit::Exit(MeasureExit {
          node_id,
          width: layout.size.width,
          height: layout.size.height,
          local_transform,
          runs,
          child_ids: child_ids.clone(),
        }));

        for (index, child_id) in child_ids.iter().copied().enumerate().rev() {
          let mut child_path = path.clone();
          child_path.push(index);
          visits.push(TraversalVisit::Enter(TraversalEnter {
            path: child_path,
            node_id: child_id,
            transform: local_transform,
            container_size: child_container_size,
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
            unreachable!()
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
pub fn render<'g, N: Node<N>>(options: RenderOptions<'g, N>) -> Result<RgbaImage> {
  let RenderOptions {
    viewport,
    global,
    node,
    draw_debug_border,
    fetched_resources,
    stylesheets,
    keyframes,
    time_ms,
    dithering,
  } = options;
  #[cfg(feature = "css_stylesheet_parsing")]
  let parsed_stylesheets = build_stylesheets(stylesheets, keyframes);
  #[cfg(feature = "css_stylesheet_parsing")]
  let mut render_context = RenderContext::new(
    global,
    viewport,
    fetched_resources,
    parsed_stylesheets,
    RenderTime { time_ms },
  );
  #[cfg(not(feature = "css_stylesheet_parsing"))]
  let mut render_context =
    RenderContext::new(global, viewport, fetched_resources, RenderTime { time_ms });
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
      width: viewport.width.map(|value| value as f32),
      height: viewport.height.map(|value| value as f32),
    },
  )?;

  let mut image = canvas.into_inner();
  apply_dithering(&mut image, dithering);

  Ok(image)
}

/// Renders a node at a specific time on the global animation timeline.
pub fn render_at_time<'g, N: Node<N>>(
  mut options: RenderOptions<'g, N>,
  time_ms: u64,
) -> Result<RgbaImage> {
  options.time_ms = time_ms;
  render(options)
}

/// Renders the active scene for a sequential animation timeline at `time_ms`.
pub fn render_sequence_at_time<'g, N: Node<N>>(
  scenes: &[SequentialScene<'g, N>],
  time_ms: u64,
) -> Result<RgbaImage> {
  let Some((scene, local_time_ms)) = resolve_scene_at_time(scenes, time_ms) else {
    return Err(Error::InvalidViewport);
  };

  render_at_time(scene.options.clone(), local_time_ms)
}

/// Renders all frames for a sequential animation timeline at a fixed frame rate.
pub fn render_sequence_animation<'g, N: Node<N>>(
  scenes: &[SequentialScene<'g, N>],
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

fn total_sequence_duration<'g, N: Node<N>>(scenes: &[SequentialScene<'g, N>]) -> u64 {
  scenes
    .iter()
    .map(|scene| u64::from(scene.duration_ms))
    .sum::<u64>()
}

fn resolve_scene_at_time<'a, 'g, N: Node<N>>(
  scenes: &'a [SequentialScene<'g, N>],
  time_ms: u64,
) -> Option<(&'a SequentialScene<'g, N>, u64)> {
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

fn apply_transform(
  transform: &mut Affine,
  style: &ComputedStyle,
  border_box: Size<f32>,
  sizing: &Sizing,
) {
  let origin = style.transform_origin.to_point(sizing, border_box);

  // CSS Transforms Level 2 order: T(origin) * translate * rotate * scale * transform * T(-origin)
  // Ref: https://www.w3.org/TR/css-transforms-2/#ctm

  let mut local = Affine::translation(origin.x, origin.y);

  if style.translate != SpacePair::default() {
    local *= Affine::translation(
      style.translate.x.to_px(sizing, border_box.width),
      style.translate.y.to_px(sizing, border_box.height),
    );
  }

  if let Some(rotate) = style.rotate {
    local *= Affine::rotation(rotate);
  }

  if style.scale != SpacePair::default() {
    local *= Affine::scale(style.scale.x.0, style.scale.y.0);
  }

  if let Some(node_transform) = &style.transform {
    local *= Affine::from_transforms(node_transform.iter(), sizing, border_box);
  }

  local *= Affine::translation(-origin.x, -origin.y);

  *transform *= local;
}

fn get_node_mut_by_path<'a, 'g, Nodes: Node<Nodes>>(
  root: &'a mut RenderNode<'g, Nodes>,
  path: &[usize],
) -> Option<&'a mut RenderNode<'g, Nodes>> {
  let mut current = root;
  for &index in path {
    let children = current.children.as_deref_mut()?;
    current = children.get_mut(index)?;
  }
  Some(current)
}

fn collect_child_node_ids(
  layout_results: &LayoutResults,
  node_id: NodeId,
  render_child_len: usize,
) -> Result<Vec<NodeId>> {
  let layout_children = layout_results.children(node_id)?;
  let child_count = render_child_len.min(layout_children.len());
  Ok(
    layout_children
      .iter()
      .copied()
      .take(child_count)
      .collect::<Vec<_>>(),
  )
}

pub(crate) fn render_node<'g, Nodes: Node<Nodes>>(
  node: &mut RenderNode<'g, Nodes>,
  layout_results: &LayoutResults,
  node_id: NodeId,
  canvas: &mut Canvas,
  transform: Affine,
  container_size: Size<Option<f32>>,
) -> Result<()> {
  fn finish_node_render<'g, Nodes: Node<Nodes>>(
    node: &mut RenderNode<'g, Nodes>,
    canvas: &mut Canvas,
    has_constrain: bool,
    original_canvas_image: Option<RgbaImage>,
  ) -> Result<()> {
    let opacity_filter =
      (node.context.style.opacity.0 < 1.0).then_some(Filter::Opacity(node.context.style.opacity));

    if !node.context.style.filter.is_empty() || opacity_filter.is_some() {
      apply_filters(
        &mut canvas.image,
        &node.context.sizing,
        node.context.current_color,
        &mut canvas.buffer_pool,
        node
          .context
          .style
          .filter
          .iter()
          .chain(opacity_filter.iter()),
      )?;
    }

    if let Some(mut source_canvas_image) = original_canvas_image {
      overlay_image(
        &mut source_canvas_image,
        &canvas.image,
        BorderProperties::zero(),
        Affine::IDENTITY,
        ImageScalingAlgorithm::Auto,
        node.context.style.mix_blend_mode,
        &[],
        &mut canvas.mask_memory,
        &mut canvas.buffer_pool,
      );

      let isolated_image = replace(&mut canvas.image, source_canvas_image);
      canvas.buffer_pool.release_image(isolated_image);
    }

    if has_constrain {
      canvas.pop_constrain();
    }

    Ok(())
  }

  let mut visits = vec![TraversalVisit::Enter(TraversalEnter {
    path: Vec::new(),
    node_id,
    transform,
    container_size,
  })];

  while let Some(visit) = visits.pop() {
    match visit {
      TraversalVisit::Enter(TraversalEnter {
        path,
        node_id,
        mut transform,
        container_size,
      }) => {
        let Some(current) = get_node_mut_by_path(node, &path) else {
          unreachable!()
        };
        let layout = *layout_results.layout(node_id)?;

        if current.context.style.is_invisible() {
          continue;
        }

        current.context.sizing.container_size = container_size;
        transform *= Affine::translation(layout.location.x, layout.location.y);
        apply_transform(
          &mut transform,
          &current.context.style,
          layout.size,
          &current.context.sizing,
        );

        if !transform.is_invertible() {
          continue;
        }

        current.context.transform = transform;

        let constrain = CanvasConstrain::from_node(
          &current.context,
          &current.context.style,
          layout,
          transform,
          &mut canvas.mask_memory,
          &mut canvas.buffer_pool,
        )?;

        if matches!(constrain, CanvasConstrainResult::SkipRendering) {
          continue;
        }

        let has_constrain = constrain.is_some();

        if !current.context.style.backdrop_filter.is_empty() {
          let border = BorderProperties::from_context(&current.context, layout.size, layout.border);
          apply_backdrop_filter(canvas, border, layout.size, transform, &current.context)?;
        }

        let should_isolate = current.context.style.is_isolated()
          || current
            .context
            .style
            .has_non_identity_transform(layout.size, &current.context.sizing);
        let original_canvas_image = if should_isolate {
          Some(canvas.replace_new_image()?)
        } else {
          None
        };

        match constrain {
          CanvasConstrainResult::None => {
            current.draw_shell(canvas, layout)?;
          }
          CanvasConstrainResult::Some(constrain) => match constrain {
            CanvasConstrain::ClipPath { .. } | CanvasConstrain::MaskImage { .. } => {
              canvas.push_constrain(constrain);
              current.draw_shell(canvas, layout)?;
            }
            CanvasConstrain::Overflow { .. } => {
              current.draw_shell(canvas, layout)?;
              canvas.push_constrain(constrain);
            }
          },
          CanvasConstrainResult::SkipRendering => unreachable!(),
        }

        current.draw_content(canvas, layout)?;

        if current.context.draw_debug_border {
          draw_debug_border(canvas, layout, transform);
        }

        if current.should_create_inline_layout() {
          current.draw_inline(canvas, layout)?;
          finish_node_render(current, canvas, has_constrain, original_canvas_image)?;
          continue;
        }

        let Some(children) = current.children.as_deref() else {
          finish_node_render(current, canvas, has_constrain, original_canvas_image)?;
          continue;
        };

        let child_ids = collect_child_node_ids(layout_results, node_id, children.len())?;
        if child_ids.is_empty() {
          finish_node_render(current, canvas, has_constrain, original_canvas_image)?;
          continue;
        }

        visits.push(TraversalVisit::Exit(RenderExit {
          path: path.clone(),
          has_constrain,
          original_canvas_image,
        }));

        let child_container_size = Size {
          width: Some(layout.content_box_width()),
          height: Some(layout.content_box_height()),
        };

        for (index, child_id) in child_ids.into_iter().enumerate().rev() {
          let mut child_path = path.clone();
          child_path.push(index);
          visits.push(TraversalVisit::Enter(TraversalEnter {
            path: child_path,
            node_id: child_id,
            transform,
            container_size: child_container_size,
          }));
        }
      }
      TraversalVisit::Exit(RenderExit {
        path,
        has_constrain,
        original_canvas_image,
      }) => {
        let Some(current) = get_node_mut_by_path(node, &path) else {
          unreachable!()
        };
        finish_node_render(current, canvas, has_constrain, original_canvas_image)?;
      }
    };
  }

  Ok(())
}

#[cfg(feature = "css_stylesheet_parsing")]
fn build_stylesheets(stylesheets: Vec<String>, keyframes: Vec<KeyframesRule>) -> Vec<StyleSheet> {
  let mut parsed: Vec<StyleSheet> =
    StyleSheet::parse_list(stylesheets.iter().map(String::as_str)).collect();
  if !keyframes.is_empty() {
    parsed.push(StyleSheet {
      rules: Vec::new(),
      keyframes,
    });
  }
  parsed
}

#[cfg(test)]
mod tests {
  use super::{
    RenderOptionsBuilder, SequentialScene, SequentialSceneBuilder, render_sequence_animation,
    resolve_scene_at_time,
  };
  use crate::{
    GlobalContext,
    layout::{
      Viewport,
      node::{ContainerNode, NodeKind},
      style::{
        AnimationDurations, AnimationFillMode, AnimationFillModes, AnimationNames, AnimationTime,
        AnimationTimingFunction, AnimationTimingFunctions, KeyframeRule, KeyframesRule, Length::Px,
        Style, StyleDeclaration,
      },
    },
    rendering::measure_layout,
  };

  fn make_scene<'g>(global: &'g GlobalContext, duration_ms: u32) -> SequentialScene<'g, NodeKind> {
    let options_result = RenderOptionsBuilder::default()
      .global(global)
      .viewport(Viewport::new(Some(10), Some(10)))
      .node(NodeKind::Container(ContainerNode::default()))
      .build();
    assert!(options_result.is_ok());
    let Ok(options) = options_result else {
      unreachable!()
    };

    let scene_result = SequentialSceneBuilder::default()
      .duration_ms(duration_ms)
      .options(options)
      .build();
    assert!(scene_result.is_ok());
    let Ok(scene) = scene_result else {
      unreachable!()
    };
    scene
  }

  #[test]
  fn resolve_scene_at_time_uses_cumulative_durations() {
    let global = GlobalContext::default();
    let scenes = vec![make_scene(&global, 100), make_scene(&global, 200)];

    let scene = resolve_scene_at_time(&scenes, 50);
    assert!(scene.is_some());
    let Some((_, local_time)) = scene else {
      unreachable!()
    };
    assert_eq!(local_time, 50);

    let scene = resolve_scene_at_time(&scenes, 150);
    assert!(scene.is_some());
    let Some((_, local_time)) = scene else {
      unreachable!()
    };
    assert_eq!(local_time, 50);
  }

  #[test]
  fn resolve_scene_at_time_clamps_to_last_scene() {
    let global = GlobalContext::default();
    let scenes = vec![make_scene(&global, 100), make_scene(&global, 200)];

    let scene = resolve_scene_at_time(&scenes, 500);
    assert!(scene.is_some());
    let Some((_, local_time)) = scene else {
      unreachable!()
    };
    assert_eq!(local_time, 199);
  }

  #[test]
  fn render_sequence_animation_returns_no_frames_for_zero_duration_timelines() {
    let global = GlobalContext::default();
    let scenes = vec![make_scene(&global, 0)];

    let frames_result = render_sequence_animation(&scenes, 30);
    assert!(frames_result.is_ok());
    let Ok(frames) = frames_result else {
      unreachable!()
    };

    assert!(frames.is_empty());
  }

  #[test]
  fn render_sequence_animation_uses_per_frame_integer_durations() {
    let global = GlobalContext::default();
    let scenes = vec![make_scene(&global, 150)];

    let frames_result = render_sequence_animation(&scenes, 30);
    assert!(frames_result.is_ok());
    let Ok(frames) = frames_result else {
      unreachable!()
    };
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
  fn measure_layout_supports_structured_keyframes() {
    let global = GlobalContext::default();
    let node: NodeKind = ContainerNode {
      class_name: None,
      id: None,
      tag_name: Some("div".into()),
      preset: None,
      style: Some(
        Style::default()
          .with(StyleDeclaration::width(Px(100.0)))
          .with(StyleDeclaration::animation_name(AnimationNames(
            vec!["grow".to_string()].into(),
          )))
          .with(StyleDeclaration::animation_duration(AnimationDurations(
            vec![AnimationTime::from_milliseconds(1000.0)].into(),
          )))
          .with(StyleDeclaration::animation_timing_function(
            AnimationTimingFunctions(vec![AnimationTimingFunction::Linear].into()),
          ))
          .with(StyleDeclaration::animation_fill_mode(AnimationFillModes(
            vec![AnimationFillMode::Both].into(),
          ))),
      ),
      children: None,
      tw: None,
    }
    .into();

    let options_result = RenderOptionsBuilder::default()
      .global(&global)
      .viewport(Viewport::new(Some(200), Some(100)))
      .node(node)
      .keyframes(vec![KeyframesRule {
        name: "grow".to_string(),
        keyframes: vec![
          KeyframeRule {
            offsets: vec![0.0],
            declarations: Style::default()
              .with(StyleDeclaration::width(Px(100.0)))
              .into(),
          },
          KeyframeRule {
            offsets: vec![1.0],
            declarations: Style::default()
              .with(StyleDeclaration::width(Px(200.0)))
              .into(),
          },
        ],
      }])
      .time_ms(500)
      .build();
    assert!(options_result.is_ok());
    let Ok(options) = options_result else {
      unreachable!()
    };

    let layout_result = measure_layout(options);
    assert!(layout_result.is_ok());
    let Ok(layout) = layout_result else {
      unreachable!()
    };

    assert_eq!(layout.width, 150.0);
  }
}
