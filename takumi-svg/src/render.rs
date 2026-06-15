//! End-to-end SVG rendering: run takumi-core layout, walk the tree, emit SVG.

use std::collections::HashMap;
use std::io;
use std::rc::Rc;
use std::sync::Arc;

use taffy::{AbsoluteAxis, AvailableSpace, NodeId, Point, Rect, Size};
use takumi_core::{
  GlobalContext,
  context::RenderContext,
  error::Result,
  layout::{
    Viewport,
    border::{BorderProperties, BorderSide},
    inline::{InlineBoxItem, VisualInlineBox},
    node::{ImageData, Node, NodeKind},
    style::{
      BackgroundClip, BackgroundImage, BasicShape, BlendMode, BorderStyle, Color, ComputedStyle,
      Display, FillRule, Overflow, ShapeRadius, Sides, SizingContext, SpacePair, StyleSheet,
    },
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  resources::image::ImageSource,
  shadow::SizedShadow,
};
use typed_builder::TypedBuilder;

use crate::box_model::{clips_overflow, element_transform, path_data};
use crate::gradient::{emit_background_images, emit_image_layers};
use crate::image::emit_image;
use crate::text::{emit_inline_content, emit_text};
use crate::{IDENTITY, Rgba, SvgDocument};

/// Inputs for [`render`]. Built with [`SvgOptions::builder`]; only `node`,
/// `viewport`, and `global` are required. Carrying inputs in a builder struct
/// keeps new options from being breaking changes.
#[derive(TypedBuilder)]
pub struct SvgOptions<'g> {
  /// The viewport to render in.
  pub(crate) viewport: Viewport,
  /// The global context (fonts, persistent images).
  pub(crate) global: &'g GlobalContext,
  /// The root node to render.
  pub(crate) node: Node,
  /// Resources fetched externally, keyed by URL.
  #[builder(default)]
  pub(crate) fetched_resources: HashMap<Arc<str>, ImageSource>,
  /// CSS stylesheets to apply before layout.
  #[builder(default)]
  pub(crate) stylesheet: StyleSheet,
  /// Global animation time in milliseconds.
  #[builder(default = 0)]
  pub(crate) time_ms: u64,
}

/// Renders a node tree to a vector SVG string.
pub fn render(options: SvgOptions<'_>) -> Result<String> {
  let viewport = options.viewport;
  let canvas_w = viewport.size.width;
  let canvas_h = viewport.size.height;
  let context = RenderContext::new(
    options.global,
    viewport,
    options.fetched_resources,
    Rc::new(options.stylesheet),
    options.time_ms,
  );
  let root = RenderNode::from_node(&context, options.node);
  let mut tree = LayoutTree::from_render_node(&root);
  let root_id = tree.root_node_id();
  tree.compute_layout(context.sizing.viewport.into());
  let results = tree.into_results();

  let root_layout = results.layout(root_id)?;
  let width = canvas_w.map_or(root_layout.size.width, |w| w as f32);
  let height = canvas_h.map_or(root_layout.size.height, |h| h as f32);
  let mut doc = SvgDocument::new(width, height)?;
  emit_node(&root, root_id, &results, 0.0, 0.0, &mut doc)?;
  Ok(doc.render()?)
}

/// Open group tokens from [`emit_box_chrome`], to be closed (innermost first:
/// `child_group`, `outer`, then `blend`) after the box content.
struct BoxChrome {
  blend: Option<crate::GroupToken>,
  mask: Option<crate::GroupToken>,
  outer: Option<crate::GroupToken>,
  clip_group: Option<crate::GroupToken>,
  child_group: Option<crate::GroupToken>,
}

impl BoxChrome {
  /// Closes the box's groups in the correct nesting order.
  fn close(self, doc: &mut SvgDocument) -> io::Result<()> {
    if let Some(group) = self.child_group {
      doc.end_group(group)?;
    }
    if let Some(group) = self.clip_group {
      doc.end_group(group)?;
    }
    if let Some(group) = self.outer {
      doc.end_group(group)?;
    }
    if let Some(group) = self.mask {
      doc.end_group(group)?;
    }
    if let Some(group) = self.blend {
      doc.end_group(group)?;
    }
    Ok(())
  }
}

/// Maps a [`BlendMode`] to its CSS `mix-blend-mode` keyword, or `None` for the
/// default (`normal`) which needs no group.
fn blend_mode_css(mode: BlendMode) -> Option<&'static str> {
  Some(match mode {
    BlendMode::Normal => return None,
    BlendMode::Multiply => "multiply",
    BlendMode::Screen => "screen",
    BlendMode::Overlay => "overlay",
    BlendMode::Darken => "darken",
    BlendMode::Lighten => "lighten",
    BlendMode::ColorDodge => "color-dodge",
    BlendMode::ColorBurn => "color-burn",
    BlendMode::HardLight => "hard-light",
    BlendMode::SoftLight => "soft-light",
    BlendMode::Difference => "difference",
    BlendMode::Exclusion => "exclusion",
    BlendMode::Hue => "hue",
    BlendMode::Saturation => "saturation",
    BlendMode::Color => "color",
    BlendMode::Luminosity => "luminosity",
    BlendMode::PlusLighter => "plus-lighter",
    BlendMode::PlusDarker => "plus-darker",
  })
}

/// Emits a box's shared chrome — the CSS transform/opacity group, box-shadows,
/// background (rounded-clipped), and borders — then opens the overflow-clip child
/// group. `x`/`y` are the box's absolute border-box top-left. The returned group
/// tokens must be closed by the caller after emitting the box's content.
fn emit_box_chrome(
  node: &RenderNode,
  layout: &taffy::Layout,
  x: f32,
  y: f32,
  doc: &mut SvgDocument,
) -> io::Result<BoxChrome> {
  let width = layout.size.width;
  let height = layout.size.height;
  let style = &node.context.style;
  let cc = node.context.current_color;

  // mix-blend-mode composites the whole element subtree against its backdrop.
  let blend = blend_mode_css(style.mix_blend_mode)
    .map(|mode| doc.begin_blend_group(mode))
    .transpose()?;

  // mask-image attenuates the whole element subtree (alpha mask by default).
  let mask = emit_mask_group(node, x, y, width, height, doc)?;

  // CSS opacity + transform + filter apply to the element's whole subtree → one
  // group.
  let opacity = style.opacity.0;
  let transform = element_transform(&node.context, layout.size, x, y);
  let filter_ref = if !style.filter.is_empty() {
    doc.filter(&style.filter, &node.context.sizing, cc, layout.size)?
  } else {
    None
  };
  let outer = (transform.is_some() || opacity < 1.0 || filter_ref.is_some())
    .then(|| {
      doc.begin_group(
        transform.unwrap_or(IDENTITY),
        opacity,
        None,
        filter_ref.as_deref(),
      )
    })
    .transpose()?;

  // clip-path wraps the whole element (background + border + content + children).
  let clip_group = emit_clip_path_group(node, layout.size, x, y, doc)?;

  emit_box_shadows(node, layout, x, y, width, height, doc)?;

  // Border/radius geometry is reused from takumi-core (the same `BorderProperties`
  // the raster backend rasterizes) instead of being reimplemented here.
  let border = BorderProperties::from_context(&node.context, layout.size, layout.border);
  let rounded = !border.is_zero();

  // Background (clipped to the rounded border-box when radii are present).
  // `background-clip: text` paints the background into the text glyphs instead of
  // the box, so the box background is suppressed here (emitted by the text path).
  let background = style.background_color.resolve(cc);
  let has_bg_image = style
    .background_image
    .as_deref()
    .is_some_and(|images| !images.is_empty());
  let clip_text = style.background_clip == BackgroundClip::Text;
  if !clip_text && (background.0[3] != 0 || has_bg_image) {
    // The painted area is clipped per `background-clip`: the full (rounded)
    // border-box, the padding box, the content box, or the border-only ring
    // (`border-area`). Mirrors the raster backend's `draw_background`.
    let bg_clip = background_clip_path(style.background_clip, &border, layout, x, y)
      .map(|(data, even_odd)| {
        if even_odd {
          doc.clip_path_evenodd(&data)
        } else {
          doc.clip_path(&data)
        }
      })
      .transpose()?;
    let bg_group = bg_clip
      .as_deref()
      .map(|clip| doc.begin_group(IDENTITY, 1.0, Some(clip), None))
      .transpose()?;
    if background.0[3] != 0 {
      doc.rect(x, y, width, height, Rgba(background.0))?;
    }
    if let Some(images) = style.background_image.as_deref() {
      emit_background_images(images, &node.context, x, y, width, height, doc)?;
    }
    if let Some(group) = bg_group {
      doc.end_group(group)?;
    }
  }

  emit_borders(&border, x, y, layout.size, doc)?;

  emit_outline(node, layout.size, x, y, doc)?;

  // Children, clipped to the (rounded) padding box when overflow is not visible.
  // With border-radius present the raster backend clips both axes to the rounded
  // padding box regardless of the per-axis overflow values, so the rounded path is
  // used as-is. Without radius a two-value overflow (e.g. `overflow-x: hidden;
  // overflow-y: visible`) must leave the visible axis unbounded.
  let child_group = clips_overflow(style)
    .then(|| {
      let path = if rounded {
        padding_box_path_data(&border, layout.border, layout.size, x, y)
      } else {
        overflow_clip_rect_data(style, layout, x, y)
      };
      doc
        .clip_path(&path)
        .and_then(|clip| doc.begin_group(IDENTITY, 1.0, Some(&clip), None))
    })
    .transpose()?;

  Ok(BoxChrome {
    blend,
    mask,
    outer,
    clip_group,
    child_group,
  })
}

/// Emits the element's `mask-image` as an SVG `<mask>` (the mask layers painted
/// into the border box) and opens the masked group wrapping the element. CSS
/// `mask-image` defaults to alpha masking. `x`/`y` are the absolute border-box
/// top-left. Returns the open group token (closed by [`BoxChrome::close`]).
fn emit_mask_group(
  node: &RenderNode,
  x: f32,
  y: f32,
  width: f32,
  height: f32,
  doc: &mut SvgDocument,
) -> io::Result<Option<crate::GroupToken>> {
  let style = &node.context.style;
  let Some(images) = style.mask_image.as_deref() else {
    return Ok(None);
  };
  if images.is_empty() || images.iter().all(|i| matches!(i, BackgroundImage::None)) {
    return Ok(None);
  }
  if width <= 0.0 || height <= 0.0 {
    return Ok(None);
  }

  let (token, reference) = doc.begin_mask()?;
  emit_image_layers(
    images,
    &style.mask_size,
    &style.mask_position,
    &style.mask_repeat,
    &node.context,
    x,
    y,
    width,
    height,
    doc,
  )?;
  doc.end_mask(token)?;
  Ok(Some(doc.begin_masked_group(&reference)?))
}

/// Resolves `clip-path` (a `BasicShape`) against the element's border box and
/// opens a clip group wrapping the element. Mirrors the raster backend's
/// `render_clip_shape_mask` geometry. `x`/`y` are the absolute border-box
/// top-left. Returns the open group token (closed by [`BoxChrome::close`]).
fn emit_clip_path_group(
  node: &RenderNode,
  size: Size<f32>,
  x: f32,
  y: f32,
  doc: &mut SvgDocument,
) -> io::Result<Option<crate::GroupToken>> {
  let Some(shape) = node.context.style.clip_path.as_ref() else {
    return Ok(None);
  };
  let sizing = &node.context.sizing;
  let clip = match shape {
    BasicShape::Ellipse(ellipse) => {
      let cx = x + ellipse.position.0.x.to_px(sizing, size.width);
      let cy = y + ellipse.position.0.y.to_px(sizing, size.height);
      // closest/farthest-side measure each axis from the center to BOTH of its
      // sides, not just the top-left corner.
      let rx = resolve_shape_radius(
        ellipse.radius_x,
        cx - x,
        x + size.width - cx,
        sizing,
        size.width,
      );
      let ry = resolve_shape_radius(
        ellipse.radius_y,
        cy - y,
        y + size.height - cy,
        sizing,
        size.height,
      );
      doc.clip_ellipse(cx, cy, rx, ry)?
    }
    BasicShape::Inset(inset) => {
      let [top_l, right_l, bottom_l, left_l] = inset.inset.0;
      let top = top_l.to_px(sizing, size.height);
      let right = right_l.to_px(sizing, size.width);
      let bottom = bottom_l.to_px(sizing, size.height);
      let left = left_l.to_px(sizing, size.width);
      let inner = Size {
        width: (size.width - left - right).max(0.0),
        height: (size.height - top - bottom).max(0.0),
      };
      let mut border = BorderProperties::default();
      if let Some(radius) = inset.border_radius {
        border.radius = Sides(
          radius
            .0
            .map(|corner| SpacePair::from_single(corner.to_px(sizing, size.width))),
        );
      }
      let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);
      border.append_mask_commands(&mut commands, inner, Point { x: left, y: top });
      let data = path_data(&commands, [1.0, 0.0, 0.0, 1.0, x, y]);
      doc.clip_path(&data)?
    }
    BasicShape::Polygon(polygon) => {
      if polygon.coordinates.is_empty() {
        return Ok(None);
      }
      let mut data = String::new();
      for (index, coord) in polygon.coordinates.iter().enumerate() {
        let px = x + coord.x.to_px(sizing, size.width);
        let py = y + coord.y.to_px(sizing, size.height);
        let cmd = if index == 0 { 'M' } else { 'L' };
        data.push_str(&format!("{cmd}{px} {py}"));
      }
      data.push('Z');
      let even_odd = polygon.fill_rule.unwrap_or(node.context.style.clip_rule) == FillRule::EvenOdd;
      doc.clip_path_transformed(&data, even_odd, None)?
    }
    BasicShape::Path(path) => {
      let even_odd = path.fill_rule.unwrap_or(node.context.style.clip_rule) == FillRule::EvenOdd;
      let transform = format!("translate({x} {y})");
      doc.clip_path_transformed(&path.path, even_odd, Some(&transform))?
    }
  };
  Ok(Some(doc.begin_group(IDENTITY, 1.0, Some(&clip), None)?))
}

/// Resolves a [`ShapeRadius`] to pixels, mirroring the raster backend's
/// `resolve_radius` (closest/farthest measured from the resolved center).
fn resolve_shape_radius(
  radius: ShapeRadius,
  near: f32,
  far: f32,
  sizing: &SizingContext,
  full: f32,
) -> f32 {
  match radius {
    ShapeRadius::ClosestSide => near.min(far),
    ShapeRadius::FarthestSide => near.max(far),
    ShapeRadius::Length(length) => length.to_px(sizing, full),
  }
}

/// Absolute SVG path `d` for the border-box rounded rectangle, reusing core's
/// `BorderProperties` geometry (cubic-bezier corners, overlap-scaled radii).
fn border_box_path_data(border: &BorderProperties, size: Size<f32>, x: f32, y: f32) -> String {
  let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);
  border.append_mask_commands(&mut commands, size, Point::ZERO);
  path_data(&commands, [1.0, 0.0, 0.0, 1.0, x, y])
}

/// Absolute SVG path `d` for the padding-box rounded rectangle (border-box inset
/// by the border widths, with inner radii), reusing core geometry.
fn padding_box_path_data(
  border: &BorderProperties,
  border_width: Rect<f32>,
  size: Size<f32>,
  x: f32,
  y: f32,
) -> String {
  let mut inner = *border;
  inner.inset_by_border_width();
  let inner_size = Size {
    width: (size.width - border_width.left - border_width.right).max(0.0),
    height: (size.height - border_width.top - border_width.bottom).max(0.0),
  };
  let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);
  inner.append_mask_commands(
    &mut commands,
    inner_size,
    Point {
      x: border_width.left,
      y: border_width.top,
    },
  );
  path_data(&commands, [1.0, 0.0, 0.0, 1.0, x, y])
}

/// Absolute SVG path `d` for the (non-rounded) overflow clip rectangle. Each
/// clipped axis is bounded to the content box (mirroring the raster backend's
/// rectangular overflow mask); a `visible` axis is left effectively unbounded so
/// content overflows there while being clipped on the other axis.
fn overflow_clip_rect_data(
  style: &ComputedStyle,
  layout: &taffy::Layout,
  x: f32,
  y: f32,
) -> String {
  const UNBOUNDED: f32 = 1.0e6;
  let clip_x = style.overflow_x != Overflow::Visible;
  let clip_y = style.overflow_y != Overflow::Visible;

  let (left, right) = if clip_x {
    let content_left = x + layout.border.left + layout.padding.left;
    (content_left, content_left + layout.content_box_width())
  } else {
    (x - UNBOUNDED, x + layout.size.width + UNBOUNDED)
  };
  let (top, bottom) = if clip_y {
    let content_top = y + layout.border.top + layout.padding.top;
    (content_top, content_top + layout.content_box_height())
  } else {
    (y - UNBOUNDED, y + layout.size.height + UNBOUNDED)
  };

  format!("M{left} {top} H{right} V{bottom} H{left} Z")
}

/// Builds the clip path (and even-odd flag) for the background fill area per
/// `background-clip`, or `None` when no clip is needed (an unrounded
/// `border-box`, which the full border-box rect already covers). Mirrors the
/// raster backend's `draw_background` clip regions.
fn background_clip_path(
  clip: BackgroundClip,
  border: &BorderProperties,
  layout: &taffy::Layout,
  x: f32,
  y: f32,
) -> Option<(String, bool)> {
  let rounded = !border.is_zero();
  match clip {
    BackgroundClip::BorderBox => {
      rounded.then(|| (border_box_path_data(border, layout.size, x, y), false))
    }
    BackgroundClip::PaddingBox => Some((
      padding_box_path_data(border, layout.border, layout.size, x, y),
      false,
    )),
    BackgroundClip::ContentBox => Some((content_box_path_data(border, layout, x, y), false)),
    BackgroundClip::BorderArea => {
      // The border ring: the (rounded) border-box with the (rounded) padding box
      // punched out, drawn even-odd so the background shows only under the border.
      let outer = border_box_path_data(border, layout.size, x, y);
      let inner = padding_box_path_data(border, layout.border, layout.size, x, y);
      Some((format!("{outer}{inner}"), true))
    }
    // `text` is handled separately by the text path; treat anything else as the
    // full border box.
    _ => rounded.then(|| (border_box_path_data(border, layout.size, x, y), false)),
  }
}

/// Absolute SVG path `d` for the content-box rounded rectangle (border-box inset
/// by border widths and padding, with inner radii), reusing core geometry.
fn content_box_path_data(
  border: &BorderProperties,
  layout: &taffy::Layout,
  x: f32,
  y: f32,
) -> String {
  let mut inner = *border;
  inner.inset_by_border_width();
  inner.expand_by(layout.padding.map(|size| -size));
  let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);
  inner.append_mask_commands(
    &mut commands,
    layout.content_box_size(),
    Point {
      x: layout.border.left + layout.padding.left,
      y: layout.border.top + layout.padding.top,
    },
  );
  path_data(&commands, [1.0, 0.0, 0.0, 1.0, x, y])
}

/// Emits one node's box decorations and recurses into its children.
fn emit_node(
  node: &RenderNode,
  node_id: NodeId,
  results: &LayoutResults,
  parent_x: f32,
  parent_y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let Ok(layout) = results.layout(node_id) else {
    return Ok(());
  };
  let x = parent_x + layout.location.x;
  let y = parent_y + layout.location.y;

  let chrome = emit_box_chrome(node, layout, x, y, doc)?;

  // A node either establishes an inline formatting context (its anonymous text +
  // inline children are laid out as one inline run set, with inline boxes recursed
  // in flow) or paints its own content and recurses block children — never both.
  if node.should_create_inline_layout() {
    emit_inline_content(node, *layout, x, y, doc)?;
  } else {
    match node.node.as_ref().map(|n| &n.kind) {
      Some(NodeKind::Image(image)) => emit_image_node(image, node, layout, x, y, doc)?,
      Some(NodeKind::Text(text)) => emit_text(text, &node.context, *layout, x, y, doc)?,
      _ => {}
    }

    if let Ok(children) = results.box_children(node_id)
      && let Some(child_nodes) = node.children.as_deref()
    {
      // Paint children in CSS stacking order: negative `z-index` first, then
      // `z-auto`/0 (and non-positioned) in tree order, then positive `z-index`.
      // A stable sort by the effective z keeps tree order within each bucket,
      // mirroring the raster backend's stacking-context buckets.
      let is_flex_or_grid_item = matches!(
        node.context.style.display,
        Display::Flex | Display::InlineFlex | Display::Grid | Display::InlineGrid
      );
      let mut ordered: Vec<_> = children
        .iter()
        .filter_map(|child| {
          let child_node = child_nodes.get(child.render_index)?;
          let style = &child_node.context.style;
          let z = if style.participates_in_positioned_paint_bucket(is_flex_or_grid_item) {
            style.z_index.painting_order_value()
          } else {
            0
          };
          Some((z, child, child_node))
        })
        .collect();
      ordered.sort_by_key(|(z, _, _)| *z);
      for (_, child, child_node) in ordered {
        emit_node(child_node, child.node_id, results, x, y, doc)?;
      }
    }
  }

  chrome.close(doc)
}

/// Recurses into an in-flow inline box (an atomic inline element such as an
/// inline-block or replaced box) positioned by the inline layout. Mirrors the
/// raster backend's `draw_inline_box`: lay the child subtree out fresh at the box
/// size, then emit it at the box's absolute origin. `container_x`/`container_y`
/// are the container's border-box top-left; the box is offset by the container's
/// border/padding plus the inline-resolved position.
pub(crate) fn emit_inline_box(
  inline_box: &VisualInlineBox,
  item: &InlineBoxItem<'_, '_>,
  container_layout: taffy::Layout,
  container_x: f32,
  container_y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let node = item.render_node;
  if node.context.style.opacity.0 == 0.0 {
    return Ok(());
  }

  // Inline boxes are positioned relative to the container's padding box.
  let box_x =
    container_x + container_layout.border.left + container_layout.padding.left + inline_box.x;
  let box_y =
    container_y + container_layout.border.top + container_layout.padding.top + inline_box.y;

  if node.participates_as_inline_box() {
    // Atomic inline box (inline-block / float): lay the subtree out fresh at the
    // box's content size and recurse, mirroring the raster backend.
    let subtree = node.clone();
    let mut tree = LayoutTree::from_render_node(&subtree);
    let inline_width =
      (inline_box.width - item.margin.grid_axis_sum(AbsoluteAxis::Horizontal)).max(0.0);
    let inline_height =
      (inline_box.height - item.margin.grid_axis_sum(AbsoluteAxis::Vertical)).max(0.0);
    tree.compute_layout(Size {
      width: AvailableSpace::Definite(inline_width),
      height: AvailableSpace::Definite(inline_height),
    });
    let results = tree.into_results();
    let root_id = results.root_node_id();
    return emit_node(
      &subtree,
      root_id,
      &results,
      box_x + item.margin.left,
      box_y + item.margin.top,
      doc,
    );
  }

  // Replaced / non-atomic inline box (e.g. an inline image): paint its chrome and
  // own content at the item's size — it has no in-flow layout to recompute.
  let box_layout: taffy::Layout = item.into();
  let chrome = emit_box_chrome(node, &box_layout, box_x, box_y, doc)?;

  match node.node.as_ref().map(|n| &n.kind) {
    Some(NodeKind::Image(image)) => emit_image_node(image, node, &box_layout, box_x, box_y, doc)?,
    Some(NodeKind::Text(text)) => emit_text(text, &node.context, box_layout, box_x, box_y, doc)?,
    _ => {}
  }

  chrome.close(doc)
}

/// Emits an image node's content into its content box. When the element has a
/// border-radius, the replaced content is clipped to the rounded **padding box**
/// (border-box inset by the border widths), mirroring the raster backend's
/// `draw_image` which masks the image with `BorderProperties::inset_by_border_width`.
/// `x`/`y` are the element's absolute border-box top-left.
fn emit_image_node(
  image: &ImageData,
  node: &RenderNode,
  layout: &taffy::Layout,
  x: f32,
  y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  // `x`/`y` are the element's absolute border-box top-left; the content box is
  // inset by the border and padding (not `content_box_x`, which also folds in the
  // element's own `location` relative to its parent).
  let ix = x + layout.border.left + layout.padding.left;
  let iy = y + layout.border.top + layout.padding.top;
  let (iw, ih) = (layout.content_box_width(), layout.content_box_height());

  let border = BorderProperties::from_context(&node.context, layout.size, layout.border);
  if border.is_zero() {
    return emit_image(image, &node.context, ix, iy, iw, ih, doc);
  }

  let path = padding_box_path_data(&border, layout.border, layout.size, x, y);
  let clip = doc.clip_path(&path)?;
  let group = doc.begin_group(IDENTITY, 1.0, Some(&clip), None)?;
  emit_image(image, &node.context, ix, iy, iw, ih, doc)?;
  doc.end_group(group)
}

/// Emits the element's borders, reusing takumi-core's `BorderProperties` geometry.
/// A uniform-color solid border becomes one even-odd "ring" (outer rounded
/// border-box minus inner rounded padding-box). Mixed per-side colors clip that
/// ring and fill each side's polygon in its own color (diagonal corner split).
/// Uniform `dashed`/`dotted`/`double` borders stroke a centerline rounded-rect.
/// 3D styles (groove/ridge/inset/outset) approximate as a solid fill. `x`/`y` are
/// the absolute border-box top-left; `size` is the border-box size.
fn emit_borders(
  border: &BorderProperties,
  x: f32,
  y: f32,
  size: Size<f32>,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  if !border.has_visible_sides() {
    return Ok(());
  }

  let matrix = [1.0, 0.0, 0.0, 1.0, x, y];

  // Uniform dashed/dotted/double borders need stroke-based rendering, not fills.
  if let Some(color) = border.has_uniform_visible_color() {
    let width = border.width.top;
    if border.is_uniform_all_sides_style(BorderStyle::Dashed) {
      return emit_stroked_border(border, color, width, matrix, size, BorderStyle::Dashed, doc);
    }
    if border.is_uniform_all_sides_style(BorderStyle::Dotted) {
      return emit_stroked_border(border, color, width, matrix, size, BorderStyle::Dotted, doc);
    }
    if border.is_uniform_all_sides_style(BorderStyle::Double) {
      return emit_double_border(border, color, width, matrix, size, doc);
    }
  }

  let mut ring = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);
  border.append_border_ring_commands(&mut ring, size);
  let ring_data = path_data(&ring, matrix);

  // Single color across every drawn side → one ring fill.
  if let Some(color) = border.has_uniform_visible_color() {
    if color.0[3] != 0 {
      return doc.path_evenodd(&ring_data, Rgba(color.0));
    }
    return Ok(());
  }

  // Mixed per-side colors: clip to the ring and fill each side's polygon. The
  // polygons partition the border box corner-to-corner (diagonal corner split),
  // so the clip rounds the corners and each side keeps its own color.
  let clip = doc.clip_path_evenodd(&ring_data)?;
  let group = doc.begin_group(IDENTITY, 1.0, Some(&clip), None)?;
  for (side, width, color) in [
    (BorderSide::Top, border.width.top, border.color.top),
    (BorderSide::Right, border.width.right, border.color.right),
    (BorderSide::Bottom, border.width.bottom, border.color.bottom),
    (BorderSide::Left, border.width.left, border.color.left),
  ] {
    if width <= 0.0 || color.0[3] == 0 {
      continue;
    }
    let mut polygon = Vec::new();
    border.append_side_polygon_commands_at(side, &mut polygon, size, Point::ZERO);
    doc.path(&path_data(&polygon, matrix), Rgba(color.0))?;
  }
  doc.end_group(group)
}

/// Emits the CSS `outline` as a ring around the border-box, expanded outward by
/// `outline-offset + outline-width`. `outline` does not affect layout; it follows
/// the border-radius. Mirrors the raster backend's `draw_outline`: a uniform
/// `BorderProperties` (outline width/color/style on all four sides, radii from the
/// element) drawn on an expanded box, so all border styles (solid/dashed/dotted/
/// double, and the 3D approximations) are reused from [`emit_borders`].
fn emit_outline(
  node: &RenderNode,
  size: Size<f32>,
  x: f32,
  y: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let style = &node.context.style;
  if !style.outline_style.is_rendered() {
    return Ok(());
  }
  let sizing = &node.context.sizing;
  let width = style.outline_width.to_px(sizing, size.width).max(0.0);
  if width <= 0.0 {
    return Ok(());
  }
  let color = style.outline_color.resolve(node.context.current_color);
  if color.0[3] == 0 {
    return Ok(());
  }
  let offset = style.outline_offset.to_px(sizing, size.width);

  let mut outline = BorderProperties {
    width: Sides([width; 4]).into(),
    color: Sides([color; 4]).into(),
    style: Sides([style.outline_style; 4]).into(),
    image_rendering: style.image_rendering,
    radius: BorderProperties::resolve_radius_part(&node.context, size),
  };
  let grow = offset + width;
  outline.expand_by(Rect {
    top: grow,
    right: grow,
    bottom: grow,
    left: grow,
  });
  let outer_size = Size {
    width: size.width + 2.0 * grow,
    height: size.height + 2.0 * grow,
  };
  emit_borders(&outline, x - grow, y - grow, outer_size, doc)
}

/// Builds the centerline rounded-rect path (border box inset by `inset` on each
/// side, radii shrunk by `inset`) for stroking dashed/dotted/ring borders.
fn centerline_path(
  border: &BorderProperties,
  inset: f32,
  matrix: [f32; 6],
  size: Size<f32>,
) -> String {
  let mut shrunk = *border;
  shrunk.expand_by(Rect {
    top: -inset,
    right: -inset,
    bottom: -inset,
    left: -inset,
  });
  let inner = Size {
    width: (size.width - 2.0 * inset).max(0.0),
    height: (size.height - 2.0 * inset).max(0.0),
  };
  let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);
  shrunk.append_mask_commands(&mut commands, inner, Point { x: inset, y: inset });
  path_data(&commands, matrix)
}

/// Strokes a uniform dashed/dotted border along the border-box centerline.
fn emit_stroked_border(
  border: &BorderProperties,
  color: Color,
  width: f32,
  matrix: [f32; 6],
  size: Size<f32>,
  style: BorderStyle,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  if color.0[3] == 0 || width <= 0.0 {
    return Ok(());
  }
  let data = centerline_path(border, width / 2.0, matrix, size);
  let (dash, gap) = (3.0 * width, 2.0 * width);
  let (dasharray, linecap) = match style {
    BorderStyle::Dotted => (format!("0 {gap}"), Some("round")),
    _ => (format!("{dash} {gap}"), None),
  };
  doc.stroke_path(&data, Rgba(color.0), width, Some(&dasharray), linecap)
}

/// Approximates a uniform `double` border as two thin rings (outer third + inner
/// third of the border width).
fn emit_double_border(
  border: &BorderProperties,
  color: Color,
  width: f32,
  matrix: [f32; 6],
  size: Size<f32>,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  if color.0[3] == 0 || width <= 0.0 {
    return Ok(());
  }
  let third = width / 3.0;
  // Outer ring centered at third/2 from the outer edge.
  let outer = centerline_path(border, third / 2.0, matrix, size);
  doc.stroke_path(&outer, Rgba(color.0), third, None, None)?;
  // Inner ring centered at width - third/2 from the outer edge.
  let inner = centerline_path(border, width - third / 2.0, matrix, size);
  doc.stroke_path(&inner, Rgba(color.0), third, None, None)
}

/// Emits outset `box-shadow`s behind the element as offset, blurred rects.
/// Inset shadows are not yet supported.
fn emit_box_shadows(
  node: &RenderNode,
  layout: &taffy::Layout,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let Some(shadows) = node.context.style.box_shadow.as_ref() else {
    return Ok(());
  };
  let cc = node.context.current_color;
  for shadow in shadows.iter() {
    if shadow.inset {
      continue;
    }
    let resolved = SizedShadow::from_box_shadow(*shadow, &node.context.sizing, cc, layout.size);
    if resolved.color.0[3] == 0 {
      continue;
    }
    let sx = x + resolved.offset_x - resolved.spread_radius;
    let sy = y + resolved.offset_y - resolved.spread_radius;
    let spread_size = Size {
      width: w + 2.0 * resolved.spread_radius,
      height: h + 2.0 * resolved.spread_radius,
    };
    if spread_size.width <= 0.0 || spread_size.height <= 0.0 {
      continue;
    }
    let fill = Rgba(resolved.color.0);

    // Shadow shape = the element's rounded border-box, radii expanded by the
    // spread (reusing core geometry, matching the raster backend).
    let mut shadow = BorderProperties::from_context(&node.context, layout.size, layout.border);
    let spread = resolved.spread_radius;
    shadow.expand_by(Rect {
      top: spread,
      right: spread,
      bottom: spread,
      left: spread,
    });
    let data = border_box_path_data(&shadow, spread_size, sx, sy);

    let filter = (resolved.blur_radius > 0.0)
      .then(|| doc.blur_filter(resolved.blur_radius / 2.0))
      .transpose()?;
    if let Some(filter) = filter {
      let group = doc.begin_group(IDENTITY, 1.0, None, Some(&filter))?;
      doc.path(&data, fill)?;
      doc.end_group(group)?;
    } else {
      doc.path(&data, fill)?;
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn renders_svg_wrapper_at_viewport_size() {
    let global = GlobalContext::default();
    let svg = render(
      SvgOptions::builder()
        .node(Node::container([]))
        .viewport(Viewport::new((120, 80)))
        .global(&global)
        .build(),
    )
    .unwrap();
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("width=\"120\""));
    assert!(svg.contains("height=\"80\""));
    assert!(!svg.contains("base64"));
  }
}
