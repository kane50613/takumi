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
      Affine, Filter, ImageScalingAlgorithm, ResolvedStyle, SpacePair, apply_backdrop_filter,
      apply_filters,
    },
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  rendering::{
    BorderProperties, Canvas, CanvasConstrain, CanvasConstrainResult, RenderContext, Sizing,
    draw_debug_border, inline_drawing::get_parent_x_height, overlay_image,
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
  #[cfg(feature = "css_stylesheet_parsing")]
  let parsed_stylesheets = StyleSheet::parse_list(options.stylesheets.iter().map(String::as_str));
  #[cfg(feature = "css_stylesheet_parsing")]
  let mut render_context = RenderContext::new(
    options.global,
    options.viewport,
    options.fetched_resources,
    parsed_stylesheets,
  );
  #[cfg(not(feature = "css_stylesheet_parsing"))]
  let mut render_context =
    RenderContext::new(options.global, options.viewport, options.fetched_resources);
  render_context.draw_debug_border = options.draw_debug_border;
  let mut root = RenderNode::from_node(&render_context, options.node);
  let mut tree = LayoutTree::from_render_node(&root);
  tree.compute_layout(render_context.sizing.viewport.into());
  let layout_results = tree.into_results();

  collect_measure_result(
    &mut root,
    &layout_results,
    layout_results.root_node_id(),
    Affine::IDENTITY,
    Size {
      width: options.viewport.width.map(|value| value as f32),
      height: options.viewport.height.map(|value| value as f32),
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
  let viewport = options.viewport;
  #[cfg(feature = "css_stylesheet_parsing")]
  let parsed_stylesheets = StyleSheet::parse_list(options.stylesheets.iter().map(String::as_str));
  #[cfg(feature = "css_stylesheet_parsing")]
  let mut render_context = RenderContext::new(
    options.global,
    viewport,
    options.fetched_resources,
    parsed_stylesheets,
  );
  #[cfg(not(feature = "css_stylesheet_parsing"))]
  let mut render_context = RenderContext::new(options.global, viewport, options.fetched_resources);
  render_context.draw_debug_border = options.draw_debug_border;

  let mut root = RenderNode::from_node(&render_context, options.node);
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

  Ok(canvas.into_inner())
}

fn apply_transform(
  transform: &mut Affine,
  style: &ResolvedStyle,
  border_box: Size<f32>,
  sizing: &Sizing,
) {
  let transform_origin = style.transform_origin.unwrap_or_default();
  let origin = transform_origin.to_point(sizing, border_box);

  // CSS Transforms Level 2 order: T(origin) * translate * rotate * scale * transform * T(-origin)
  // Ref: https://www.w3.org/TR/css-transforms-2/#ctm

  let mut local = Affine::translation(origin.x, origin.y);

  let translate = style.translate();
  if translate != SpacePair::default() {
    local *= Affine::translation(
      translate.x.to_px(sizing, border_box.width),
      translate.y.to_px(sizing, border_box.height),
    );
  }

  if let Some(rotate) = style.rotate {
    local *= Affine::rotation(rotate);
  }

  let scale = style.scale();
  if scale != SpacePair::default() {
    local *= Affine::scale(scale.x.0, scale.y.0);
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
