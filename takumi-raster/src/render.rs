use std::{collections::HashMap, rc::Rc, sync::Arc};

use serde::Serialize;
use takumi_core::{
  geometry::{AvailableSpace, ComputedLayout as Layout, NodeId, Size},
  layout::node::NodeKind,
  scene::build_stacking_contexts,
  style::{ComputedStyle, Display, Lang},
};
use typed_builder::TypedBuilder;

use crate::{
  AnimationFrame, Bitmap, Canvas, DitheringAlgorithm, Error, Fonts, RenderContext, Result,
  SizedFontStyle, apply_dithering,
  layout::{
    inline::{
      InlineItem, InlineLayoutMode, InlineLayoutRequest, collect_inline_items, create_inline_layout,
    },
    node::Node,
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  resources::{font::FontsSnapshot, image::ImageSource},
  stacking_context::paint_context,
  style::{Affine, FontFamily, SizingContext, StyleSheet},
  viewport::Viewport,
};

#[derive(Clone, TypedBuilder)]
/// Options for rendering a node, built with [`RenderOptions::builder`].
pub struct RenderOptions<'g> {
  /// The viewport to render the node in.
  pub(crate) viewport: Viewport,
  /// The font context.
  pub(crate) fonts: &'g Fonts,
  /// The node to render.
  pub(crate) node: Node,
  /// Whether to draw debug borders.
  #[builder(default = false)]
  pub(crate) draw_debug_border: bool,
  /// Pre-decoded images keyed by `src`, resolved when a node references that URL.
  #[builder(default)]
  pub(crate) images: HashMap<Arc<str>, ImageSource>,
  /// CSS stylesheets to apply before layout/rendering.
  #[builder(default)]
  pub(crate) stylesheet: Arc<StyleSheet>,
  /// Global animation time in milliseconds.
  #[builder(default = 0)]
  pub(crate) time_ms: u64,
  /// Output dithering algorithm. Only used by encoding frontends.
  #[builder(default)]
  pub(crate) dithering: DitheringAlgorithm,
  /// Per-render font fallback chain (family names in order). `None` uses all
  /// registered families in registration order.
  #[builder(default)]
  pub(crate) font_families: Option<FontFamily>,
  /// Default BCP-47 language tag applied to the root, inherited by nodes without
  /// their own `lang`. Drives locale-aware shaping and line-breaking.
  #[builder(default)]
  pub(crate) lang: Option<Lang>,
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

  /// Returns the font context.
  pub fn fonts(&self) -> &'g Fonts {
    self.fonts
  }

  /// Returns the CSS stylesheet applied before layout.
  pub fn stylesheet(&self) -> &Arc<StyleSheet> {
    &self.stylesheet
  }

  /// Returns the pre-decoded images keyed by `src`.
  pub fn images(&self) -> &HashMap<Arc<str>, ImageSource> {
    &self.images
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
pub fn measure<'g>(options: RenderOptions<'g>) -> Result<MeasuredNode> {
  let RenderOptions {
    viewport,
    fonts,
    node,
    draw_debug_border,
    images,
    stylesheet,
    time_ms,
    dithering: _,
    font_families,
    lang,
  } = options;

  let render_context = RenderContext::builder()
    .fonts(fonts.snapshot_with_fallbacks(font_families.as_ref()))
    .sizing(SizingContext::builder().viewport(viewport).build())
    .images(Rc::new(images))
    .stylesheet(stylesheet)
    .time_ms(time_ms)
    .draw_debug_border(draw_debug_border)
    .style(Box::new(ComputedStyle {
      lang,
      font_family: font_families.unwrap_or_default(),
      ..Default::default()
    }))
    .build();

  let mut root = RenderNode::from_node(&render_context, node);
  let mut tree = LayoutTree::from_render_node(&root);

  tree.compute_layout(render_context.sizing.viewport.into());

  let layout_results = tree.into_results();

  collect_measure_result(
    &mut root,
    &layout_results,
    NodeId::ROOT,
    Affine::IDENTITY,
    Size {
      width: viewport.size.width.map(|value| value as f32),
      height: viewport.size.height.map(|value| value as f32),
    },
  )
}

fn collect_measure_result(
  node: &mut RenderNode,
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
        let Some(current) = node.node_at_path_mut(&path) else {
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
        node_transforms.insert(node_id, local_transform);

        let mut children = Vec::new();
        let mut runs = Vec::new();

        if current.should_create_inline_layout() {
          let font_style = SizedFontStyle::from_style(&current.context.style, &current.context);
          let built = create_inline_layout(InlineLayoutRequest::in_available_space(
            collect_inline_items(current),
            Size {
              width: AvailableSpace::Definite(layout.content_box_width()),
              height: AvailableSpace::Definite(layout.content_box_height()),
            },
            Size::NONE,
            &font_style,
            &current.context,
            InlineLayoutMode::Measure,
          ));
          let (measured_runs, measured_boxes) = built.measure_runs(layout);
          runs.extend(measured_runs.into_iter().map(|run| MeasuredTextRun {
            text: run.text.to_string(),
            x: run.x,
            y: run.y,
            width: run.width,
            height: run.height,
          }));
          children.extend(measured_boxes.into_iter().map(|inline_box| {
            let inline_transform =
              local_transform * Affine::translation(inline_box.x, inline_box.y);
            MeasuredNode {
              width: inline_box.width,
              height: inline_box.height,
              transform: inline_transform.to_cols_array(),
              children: Vec::new(),
              runs: Vec::new(),
            }
          }));

          measured_by_node_id.insert(
            usize::from(node_id),
            create_measured_node(layout, local_transform, children, runs),
          );
          continue;
        }

        // Paint always draws a text node's own text, even when generated
        // content gave it box children; its runs sit beside those children.
        if current.context.style.display != Display::None
          && !current.has_anonymous_text_item_child()
          && let Some(text) = current.node.as_ref().and_then(|node| match &node.kind {
            NodeKind::Text(data) => Some(data.text.as_str()),
            _ => None,
          })
        {
          let font_style = SizedFontStyle::from_style(&current.context.style, &current.context);
          let built = create_inline_layout(InlineLayoutRequest::in_available_space(
            vec![InlineItem::Text {
              text: text.into(),
              context: &current.context,
              link: None,
              decorations: None,
            }],
            Size {
              width: AvailableSpace::Definite(layout.content_box_width()),
              height: AvailableSpace::Definite(layout.content_box_height()),
            },
            Size::NONE,
            &font_style,
            &current.context,
            InlineLayoutMode::Measure,
          ));
          let (measured_runs, _) = built.measure_runs(layout);
          runs.extend(measured_runs.into_iter().map(|run| MeasuredTextRun {
            text: run.text.to_string(),
            x: run.x,
            y: run.y,
            width: run.width,
            height: run.height,
          }));
        }

        if current.children.is_none() {
          measured_by_node_id.insert(
            usize::from(node_id),
            create_measured_node(layout, local_transform, children, runs),
          );
          continue;
        }

        let layout_children = layout_results.box_children(node_id)?;
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
            return Err(Error::InvalidLayoutNode(child_id.into()));
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
    .ok_or(Error::InvalidLayoutNode(node_id.into()))
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
pub fn render<'g>(options: RenderOptions<'g>) -> Result<Bitmap> {
  let RenderOptions {
    viewport,
    fonts,
    node,
    draw_debug_border,
    images,
    stylesheet,
    time_ms,
    dithering,
    font_families,
    lang,
  } = options;

  let render_context = RenderContext::builder()
    .fonts(fonts.snapshot_with_fallbacks(font_families.as_ref()))
    .sizing(SizingContext::builder().viewport(viewport).build())
    .images(Rc::new(images))
    .stylesheet(stylesheet)
    .time_ms(time_ms)
    .draw_debug_border(draw_debug_border)
    .style(Box::new(ComputedStyle {
      lang,
      font_family: font_families.unwrap_or_default(),
      ..Default::default()
    }))
    .build();

  render_with_context(render_context, node, viewport, dithering)
}

/// Rasterizes `node` under an already-built [`RenderContext`]. The context
/// carries the font snapshot, images, and stylesheet, so animation frames share
/// one snapshot instead of re-snapshotting per frame.
fn render_with_context(
  render_context: RenderContext,
  node: Node,
  viewport: Viewport,
  dithering: DitheringAlgorithm,
) -> Result<Bitmap> {
  let mut root = RenderNode::from_node(&render_context, node);
  let mut tree = LayoutTree::from_render_node(&root);

  tree.compute_layout(render_context.sizing.viewport.into());

  let layout_results = tree.into_results();
  let root_node_id = NodeId::ROOT;
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

  let mut canvas = Canvas::try_new(root_size).ok_or(Error::InvalidViewport)?;

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

  Ok(Bitmap::from_rgba(image))
}

/// A scene with its per-frame-invariant render state precomputed: the font
/// snapshot and the shared image and stylesheet handles do not change between
/// frames of the same scene, so they are built once and cheaply cloned per
/// frame instead of rebuilt (and the whole option tree deep-cloned) each time.
///
/// This is the seam where wider per-frame layout reuse would later live.
pub(crate) struct PreparedScene<'a, 'g> {
  scene: &'a SequentialScene<'g>,
  fonts: FontsSnapshot,
  images: Rc<HashMap<Arc<str>, ImageSource>>,
  stylesheet: Arc<StyleSheet>,
}

impl<'a, 'g> PreparedScene<'a, 'g> {
  fn new(scene: &'a SequentialScene<'g>) -> Self {
    let options = &scene.options;

    Self {
      fonts: options
        .fonts
        .snapshot_with_fallbacks(options.font_families.as_ref()),
      images: Rc::new(options.images.clone()),
      stylesheet: options.stylesheet.clone(),
      scene,
    }
  }

  fn render_at_time(&self, time_ms: u64) -> Result<Bitmap> {
    let options = &self.scene.options;

    let render_context = RenderContext::builder()
      .fonts(self.fonts.clone())
      .sizing(SizingContext::builder().viewport(options.viewport).build())
      .images(self.images.clone())
      .stylesheet(self.stylesheet.clone())
      .time_ms(time_ms)
      .draw_debug_border(options.draw_debug_border)
      .style(Box::new(ComputedStyle {
        lang: options.lang,
        font_family: options.font_families.clone().unwrap_or_default(),
        ..Default::default()
      }))
      .build();

    render_with_context(
      render_context,
      options.node.clone(),
      options.viewport,
      options.dithering,
    )
  }
}

/// Precomputes the per-frame-invariant state for every scene in a timeline.
pub(crate) fn prepare_scenes<'a, 'g>(
  scenes: &'a [SequentialScene<'g>],
) -> Vec<PreparedScene<'a, 'g>> {
  scenes.iter().map(PreparedScene::new).collect()
}

fn resolve_prepared_at_time<'p, 'a, 'g>(
  prepared: &'p [PreparedScene<'a, 'g>],
  time_ms: u64,
) -> Option<(&'p PreparedScene<'a, 'g>, u64)> {
  resolve_at_time(
    prepared.len(),
    |index| prepared[index].scene.duration_ms,
    time_ms,
  )
  .map(|(index, local_time_ms)| (&prepared[index], local_time_ms))
}

/// A frame's start offset and displayed duration, both in milliseconds. Computed
/// from the timeline and frame rate without rendering any pixels.
#[derive(Clone, Copy)]
pub(crate) struct FrameSpan {
  pub(crate) start_ms: u64,
  pub(crate) duration_ms: u32,
}

/// The frame schedule for a timeline at a fixed frame rate: one [`FrameSpan`] per
/// visible frame, dropping any that round to a zero-millisecond duration.
pub(crate) fn frame_spans<'g>(scenes: &[SequentialScene<'g>], fps: u32) -> Vec<FrameSpan> {
  if scenes.is_empty() || fps == 0 {
    return Vec::new();
  }

  let total_duration_ms = total_sequence_duration(scenes);
  if total_duration_ms == 0 {
    return Vec::new();
  }

  let frame_count = total_duration_ms
    .saturating_mul(u64::from(fps))
    .div_ceil(1000);

  (0..frame_count)
    .filter_map(|frame_index| {
      let start_ms = frame_index * 1000 / u64::from(fps);
      let end_ms = ((frame_index + 1) * 1000 / u64::from(fps)).min(total_duration_ms);
      let duration_ms = end_ms.saturating_sub(start_ms);
      (duration_ms != 0).then_some(FrameSpan {
        start_ms,
        duration_ms: duration_ms as u32,
      })
    })
    .collect()
}

/// Renders one frame of the timeline for the given [`FrameSpan`].
pub(crate) fn render_frame(
  prepared: &[PreparedScene<'_, '_>],
  span: FrameSpan,
) -> Result<AnimationFrame> {
  let Some((scene, local_time_ms)) = resolve_prepared_at_time(prepared, span.start_ms) else {
    return Err(Error::InvalidViewport);
  };

  let image = scene.render_at_time(local_time_ms)?;
  Ok(AnimationFrame::new(image, span.duration_ms))
}

/// Renders all frames for a sequential animation timeline at a fixed frame rate.
///
/// Holds every frame in memory. To bound memory, stream straight into an encoder
/// with [`write_animation`](crate::write_animation) instead.
pub fn render_animation<'g>(
  scenes: &[SequentialScene<'g>],
  fps: u32,
) -> Result<Vec<AnimationFrame>> {
  let prepared = prepare_scenes(scenes);

  frame_spans(scenes, fps)
    .into_iter()
    .map(|span| render_frame(&prepared, span))
    .collect()
}

fn total_sequence_duration<'g>(scenes: &[SequentialScene<'g>]) -> u64 {
  scenes
    .iter()
    .map(|scene| u64::from(scene.duration_ms))
    .sum::<u64>()
}

/// Resolves which scene of a `count`-long timeline is active at `time_ms` and the
/// time offset within it, using `duration_ms(index)` for each scene's length.
/// Times past the end clamp to the last scene's final millisecond.
fn resolve_at_time(
  count: usize,
  duration_ms: impl Fn(usize) -> u32,
  time_ms: u64,
) -> Option<(usize, u64)> {
  if count == 0 {
    return None;
  }

  let total_ms = (0..count)
    .map(|index| u64::from(duration_ms(index)))
    .sum::<u64>();
  let clamped_time_ms = time_ms.min(total_ms.saturating_sub(1));
  let mut elapsed_ms = 0_u64;

  for index in 0..count {
    let next_elapsed_ms = elapsed_ms + u64::from(duration_ms(index));
    if clamped_time_ms < next_elapsed_ms {
      return Some((index, clamped_time_ms - elapsed_ms));
    }
    elapsed_ms = next_elapsed_ms;
  }

  let last = count - 1;
  Some((last, u64::from(duration_ms(last).saturating_sub(1))))
}

#[cfg(test)]
fn resolve_scene_at_time<'a, 'g>(
  scenes: &'a [SequentialScene<'g>],
  time_ms: u64,
) -> Option<(&'a SequentialScene<'g>, u64)> {
  resolve_at_time(scenes.len(), |index| scenes[index].duration_ms, time_ms)
    .map(|(index, local_time_ms)| (&scenes[index], local_time_ms))
}

pub(crate) fn render_node(
  node: &mut RenderNode,
  layout_results: &LayoutResults,
  node_id: NodeId,
  canvas: &mut Canvas,
  transform: Affine,
  container_size: Size<Option<f32>>,
) -> Result<()> {
  let contexts = build_stacking_contexts(
    node,
    layout_results,
    node_id,
    transform,
    (container_size.width, container_size.height),
  )?;
  paint_context(node, &contexts, layout_results, canvas, 0)
}

#[cfg(test)]
mod tests {
  use image::Rgba;

  use super::{RenderOptions, SequentialScene, render, render_animation, resolve_scene_at_time};
  use crate::{
    Fonts,
    layout::node::Node,
    measure,
    style::{
      AnimationFillMode, AnimationTime, AnimationTimingFunction, Color, ColorInput, Display,
      KeyframeRule, KeyframesRule, Length, Length::Px, Position, Style, StyleDeclaration,
      StyleSheet,
    },
    viewport::Viewport,
  };

  fn make_scene<'g>(fonts: &'g Fonts, duration_ms: u32) -> SequentialScene<'g> {
    let options = RenderOptions::builder()
      .fonts(fonts)
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
    let fonts = Fonts::default();
    let scenes = vec![make_scene(&fonts, 100), make_scene(&fonts, 200)];

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
    let fonts = Fonts::default();
    let scenes = vec![make_scene(&fonts, 100), make_scene(&fonts, 200)];

    let scene = resolve_scene_at_time(&scenes, 500);
    assert!(scene.is_some());
    let local_time = scene.map_or(0, |(_, local_time)| local_time);
    assert_eq!(local_time, 199);
  }

  #[test]
  fn render_sequence_animation_returns_no_frames_for_zero_duration_timelines() {
    let fonts = Fonts::default();
    let scenes = vec![make_scene(&fonts, 0)];

    let frames_result = render_animation(&scenes, 30);
    assert!(frames_result.is_ok());
    let frames = frames_result.unwrap_or_default();

    assert!(frames.is_empty());
  }

  #[test]
  fn oversized_viewport_errors_instead_of_silent_1x1() {
    let fonts = Fonts::default();
    // A width whose row byte length (width * 4) overflows u32, so the backing
    // pixmap cannot allocate.
    let options = RenderOptions::builder()
      .fonts(&fonts)
      .viewport(Viewport::new((2_000_000_000, 1)))
      .node(Node::container([]))
      .build();

    assert!(matches!(
      render(options),
      Err(crate::Error::InvalidViewport)
    ));
  }

  #[test]
  fn viewport_over_pixel_budget_errors_before_allocating() {
    let fonts = Fonts::default();
    let options = RenderOptions::builder()
      .fonts(&fonts)
      .viewport(Viewport::new((4097, 4096)))
      .node(Node::container([]))
      .build();

    assert!(matches!(
      render(options),
      Err(crate::Error::InvalidViewport)
    ));
  }

  #[test]
  fn ordinary_viewport_still_renders() {
    let fonts = Fonts::default();
    let options = RenderOptions::builder()
      .fonts(&fonts)
      .viewport(Viewport::new((100, 100)))
      .node(Node::container([]))
      .build();

    let bitmap = render(options).unwrap();
    assert_eq!((bitmap.width(), bitmap.height()), (100, 100));
  }

  #[test]
  fn write_animation_streams_the_same_bytes_as_render_then_encode() -> crate::Result<()> {
    use std::borrow::Cow;

    use crate::{
      AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFormat,
      write_animated_gif, write_animated_png, write_animated_webp, write_animation,
    };

    let fonts = Fonts::default();
    let scenes = vec![make_scene(&fonts, 100), make_scene(&fonts, 100)];
    let fps = 30;
    let frames = render_animation(&scenes, fps)?;
    assert!(!frames.is_empty());

    let mut eager = Vec::new();
    write_animated_gif(
      Cow::Owned(frames.clone()),
      &mut eager,
      AnimatedGifOptions::default(),
    )?;
    let mut streamed = Vec::new();
    write_animation(
      &scenes,
      fps,
      AnimationFormat::Gif(AnimatedGifOptions::default()),
      &mut streamed,
    )?;
    assert_eq!(eager, streamed, "gif");

    let mut eager = Vec::new();
    write_animated_png(&frames, &mut eager, AnimatedPngOptions::default())?;
    let mut streamed = Vec::new();
    write_animation(
      &scenes,
      fps,
      AnimationFormat::Apng(AnimatedPngOptions::default()),
      &mut streamed,
    )?;
    assert_eq!(eager, streamed, "apng");

    let mut eager = Vec::new();
    write_animated_webp(
      Cow::Owned(frames.clone()),
      &mut eager,
      AnimatedWebpOptions::default(),
    )?;
    let mut streamed = Vec::new();
    write_animation(
      &scenes,
      fps,
      AnimationFormat::WebP(AnimatedWebpOptions::default()),
      &mut streamed,
    )?;
    assert_eq!(eager, streamed, "webp");

    Ok(())
  }

  #[test]
  fn write_animation_rejects_frame_rate_above_format_cap() {
    use crate::{AnimatedGifOptions, AnimatedWebpOptions, AnimationFormat, Error, write_animation};

    let fonts = Fonts::default();
    let scenes = vec![make_scene(&fonts, 100)];

    let mut sink = Vec::new();
    let over_webp = write_animation(
      &scenes,
      91,
      AnimationFormat::WebP(AnimatedWebpOptions::default()),
      &mut sink,
    );
    assert!(matches!(
      over_webp,
      Err(Error::AnimationFrameRateTooHigh {
        fps: 91,
        max_fps: 90
      })
    ));
    assert!(sink.is_empty(), "cap must reject before writing bytes");

    let mut sink = Vec::new();
    let over_gif = write_animation(
      &scenes,
      51,
      AnimationFormat::Gif(AnimatedGifOptions::default()),
      &mut sink,
    );
    assert!(matches!(
      over_gif,
      Err(Error::AnimationFrameRateTooHigh {
        fps: 51,
        max_fps: 50
      })
    ));

    let mut sink = Vec::new();
    let at_cap = write_animation(
      &scenes,
      90,
      AnimationFormat::WebP(AnimatedWebpOptions::default()),
      &mut sink,
    );
    assert!(at_cap.is_ok());
    assert!(!sink.is_empty());
  }

  #[test]
  fn render_sequence_animation_uses_per_frame_integer_durations() {
    let fonts = Fonts::default();
    let scenes = vec![make_scene(&fonts, 150)];

    let frames_result = render_animation(&scenes, 30);
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
  fn measure_layout_supports_structured_keyframes() {
    let fonts = Fonts::default();
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
      .fonts(&fonts)
      .viewport(Viewport::new((200, 100)))
      .node(node)
      .stylesheet(
        StyleSheet::from(vec![KeyframesRule {
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
        }])
        .into(),
      )
      .time_ms(500)
      .build();

    let layout_result = measure(options);
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
    let fonts = Fonts::default();
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
      .fonts(&fonts)
      .viewport(Viewport::new((200, 200)))
      .node(root)
      .build();

    let layout = match measure(options) {
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
    let fonts = Fonts::default();
    let options = RenderOptions::builder()
      .fonts(&fonts)
      .viewport(Viewport::new((256, 256)))
      .node(node.clone())
      .build();
    let measured = match measure(options.clone()) {
      Ok(measured) => measured,
      Err(_) => return,
    };
    assert_eq!(measured.children.len(), 1);
    assert_eq!(measured.children[0].width, 128.0);
    assert_eq!(measured.children[0].height, 128.0);

    let rendered = match render(options) {
      Ok(rendered) => rendered.into_rgba(),
      Err(_) => return,
    };

    let top_left = rendered.get_pixel(10, 10);
    let bottom_right = rendered.get_pixel(220, 220);

    assert_eq!(top_left, &Rgba([255, 0, 0, 255]));
    assert_eq!(bottom_right, &Rgba([11, 16, 32, 255]));
  }
}
