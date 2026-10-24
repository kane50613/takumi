//! Flattens a usvg tree into the drawing ops every backend draws an SVG image from.

use std::{iter::repeat_n, sync::Arc};

use tiny_skia_path::{PathSegment, Transform};

#[cfg(feature = "gif")]
use crate::resources::image_decoder::decode_gif_frames;
use crate::{
  geometry::{PathCommand, Point},
  resources::{glyph::ResolvedGlyph, image_buffer::ImageBuffer, image_decoder::decode_image},
  resvg::usvg::{
    self, ClipPath, Group, ImageKind, ImageRendering, Mask, MaskType, Node, Paint, PaintOrder,
    Tree, filter::Filter,
  },
  style::{BlendMode, ImageScalingAlgorithm},
};

/// One flattened SVG drawing instruction, in SVG canvas coordinates.
#[derive(Debug, Clone)]
pub enum SvgOp {
  /// Applies an affine transform `(sx, ky, kx, sy, tx, ty)` to nested ops.
  PushTransform([f32; 6]),
  /// Clips nested ops to a path.
  PushClip {
    /// Clip outline.
    path: Vec<PathCommand>,
    /// `true` for the even-odd fill rule, `false` for nonzero.
    evenodd: bool,
  },
  /// Blends nested ops onto the backdrop with a blend mode.
  PushBlend(BlendMode),
  /// Applies uniform opacity to nested ops as one isolated group.
  PushOpacity(f32),
  /// Masks nested ops with the rendering of `ops` (an alpha or luminance soft mask).
  PushMask {
    /// Mask content, flattened recursively.
    ops: Vec<SvgOp>,
    /// `true` to mask by luminance, `false` by alpha.
    luminance: bool,
  },
  /// Filters nested ops as one layer, in the coordinate space the layer is pushed in.
  PushFilter(SvgFilters),
  /// Closes the innermost open `Push*` layer.
  Pop,
  /// Fills and/or strokes a path.
  Draw {
    /// Path outline in the current coordinate space.
    path: Vec<PathCommand>,
    /// Fill paint, if any.
    fill: Option<SvgFill>,
    /// Stroke paint, if any.
    stroke: Option<SvgStrokeStyle>,
    /// `false` for `shape-rendering: crispEdges` and `optimizeSpeed`.
    anti_alias: bool,
  },
  /// Draws a bitmap over `(0, 0, width, height)` in the current coordinate space.
  Image {
    /// Bitmap pixels.
    data: SvgImageData,
    /// Width in user units.
    width: f32,
    /// Height in user units.
    height: f32,
    /// How the bitmap resamples.
    rendering: ImageScalingAlgorithm,
  },
}

/// The pixels of a bitmap inside an SVG.
#[derive(Debug, Clone)]
pub enum SvgImageData {
  /// PNG, JPEG or WebP bytes, as the SVG carries them.
  Encoded(Arc<Vec<u8>>),
  /// Decoded pixels, for a GIF's first frame or a bitmap glyph.
  Bitmap(ImageBuffer),
}

impl SvgImageData {
  /// The decoded pixels, or `None` when the bytes do not decode.
  pub fn decode(&self) -> Option<ImageBuffer> {
    match self {
      Self::Encoded(bytes) => decode_image(bytes).ok(),
      Self::Bitmap(buffer) => Some(buffer.clone()),
    }
  }
}

/// The SVG filters of one group, applied to its layer.
#[derive(Debug, Clone)]
pub struct SvgFilters(#[cfg_attr(not(feature = "svg-raster"), allow(dead_code))] Vec<Arc<Filter>>);

#[cfg(feature = "svg-raster")]
impl SvgFilters {
  /// The union of the filter regions in the group's coordinate space.
  pub fn region(&self) -> Option<tiny_skia::Rect> {
    let mut regions = self.0.iter().map(|filter| filter.rect().to_rect());
    let first = regions.next()?;

    regions.try_fold(first, |union, region| {
      tiny_skia::Rect::from_ltrb(
        union.left().min(region.left()),
        union.top().min(region.top()),
        union.right().max(region.right()),
        union.bottom().max(region.bottom()),
      )
    })
  }

  /// Applies each filter in order to `layer`, drawn under `transform`.
  pub fn apply(&self, transform: tiny_skia::Transform, layer: &mut tiny_skia::Pixmap) {
    for filter in &self.0 {
      crate::resvg::apply_filter(filter, transform, layer);
    }
  }
}

/// Fill style for [`SvgOp::Draw`].
#[derive(Debug, Clone)]
pub struct SvgFill {
  /// Fill paint.
  pub paint: SvgPaint,
  /// Fill opacity in `0..=1`.
  pub opacity: f32,
  /// `true` for the even-odd fill rule, `false` for nonzero.
  pub evenodd: bool,
}

/// Stroke style for [`SvgOp::Draw`].
#[derive(Debug, Clone)]
pub struct SvgStrokeStyle {
  /// Stroke paint.
  pub paint: SvgPaint,
  /// Stroke opacity in `0..=1`.
  pub opacity: f32,
  /// Stroke width in user units.
  pub width: f32,
  /// Miter limit.
  pub miter_limit: f32,
  /// Line cap.
  pub cap: SvgLineCap,
  /// Line join.
  pub join: SvgLineJoin,
  /// Dash pattern as `(array, offset)`.
  pub dash: Option<(Vec<f32>, f32)>,
}

/// Line cap for [`SvgStrokeStyle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgLineCap {
  /// Flat edge at the endpoint.
  Butt,
  /// Semicircle past the endpoint.
  Round,
  /// Half-square past the endpoint.
  Square,
}

/// Line join for [`SvgStrokeStyle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgLineJoin {
  /// Sharp corner, which becomes a bevel past the miter limit.
  Miter,
  /// Rounded corner.
  Round,
  /// Beveled corner.
  Bevel,
  /// Miter clipped at the miter limit instead of falling back to a bevel.
  MiterClip,
}

/// Paint for fills and strokes.
#[derive(Debug, Clone)]
pub enum SvgPaint {
  /// Solid sRGB color.
  Color([u8; 3]),
  /// Linear gradient from `start` to `end` in user space.
  Linear {
    /// Gradient line start.
    start: Point<f32>,
    /// Gradient line end.
    end: Point<f32>,
    /// Shared gradient parameters.
    gradient: SvgGradient,
  },
  /// Radial gradient with a focal circle.
  Radial {
    /// Center.
    center: Point<f32>,
    /// Radius.
    radius: f32,
    /// Focal point.
    focal: Point<f32>,
    /// Focal radius.
    focal_radius: f32,
    /// Shared gradient parameters.
    gradient: SvgGradient,
  },
  /// Tiling pattern: `ops` draw one `width` x `height` tile placed by `transform`.
  Pattern {
    /// Tile content, flattened recursively.
    ops: Vec<SvgOp>,
    /// Pattern space transform `(sx, ky, kx, sy, tx, ty)`.
    transform: [f32; 6],
    /// Tile width.
    width: f32,
    /// Tile height.
    height: f32,
  },
}

/// Parameters shared by both gradient kinds.
#[derive(Debug, Clone)]
pub struct SvgGradient {
  /// Extra gradient transform `(sx, ky, kx, sy, tx, ty)`.
  pub transform: [f32; 6],
  /// How the gradient extends past its bounds.
  pub spread: SvgSpreadMethod,
  /// Color stops, offsets ascending in `0..=1`.
  pub stops: Vec<SvgGradientStop>,
}

/// Gradient spread method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgSpreadMethod {
  /// Clamp to the edge stops.
  Pad,
  /// Mirror-repeat.
  Reflect,
  /// Repeat.
  Repeat,
}

/// One gradient color stop.
#[derive(Debug, Clone)]
pub struct SvgGradientStop {
  /// Offset in `0..=1`.
  pub offset: f32,
  /// sRGB color.
  pub color: [u8; 3],
  /// Stop opacity in `0..=1`.
  pub opacity: f32,
}

/// Flattens `tree` into vector ops in SVG canvas coordinates.
pub(crate) fn flatten(tree: &Tree) -> Vec<SvgOp> {
  Flattener::default().flatten(tree.root())
}

/// Flattens `group`'s children without its transform, as an `feImage` filter draws them.
#[cfg(feature = "svg-raster")]
pub(crate) fn flatten_children(group: &Group) -> Vec<SvgOp> {
  let mut flattener = Flattener::default();

  for child in group.children() {
    flattener.push_node(child);
  }
  flattener.ops
}

/// One op list being written.
#[derive(Default)]
struct Flattener {
  ops: Vec<SvgOp>,
}

impl Flattener {
  /// The ops `fill` writes into a fresh list: a mask, a clip mask or a pattern tile.
  fn nested(&self, fill: impl FnOnce(&mut Self)) -> Vec<SvgOp> {
    let mut nested = Self::default();

    fill(&mut nested);
    nested.ops
  }

  fn flatten(mut self, group: &Group) -> Vec<SvgOp> {
    self.push_group(group);
    self.ops
  }

  fn push_group(&mut self, group: &Group) {
    let mut pop_count = 0;

    if group.transform() != Transform::identity() {
      self
        .ops
        .push(SvgOp::PushTransform(transform_array(group.transform())));
      pop_count += 1;
    }

    if let Some(clip_path) = group.clip_path() {
      pop_count += self.push_clip_path(clip_path);
    }

    if let Some(mask) = group.mask() {
      let op = self.mask_op(mask);

      self.ops.push(op);
      pop_count += 1;
    }

    if group.blend_mode() != usvg::BlendMode::Normal {
      self
        .ops
        .push(SvgOp::PushBlend(convert_blend_mode(group.blend_mode())));
      pop_count += 1;
    }

    if group.opacity().get() < 1.0 {
      self.ops.push(SvgOp::PushOpacity(group.opacity().get()));
      pop_count += 1;
    }

    if !group.filters().is_empty() {
      self
        .ops
        .push(SvgOp::PushFilter(SvgFilters(group.filters().to_vec())));
      pop_count += 1;
    }

    for child in group.children() {
      self.push_node(child);
    }

    self.pop(pop_count);
  }

  /// Closes the innermost `count` open layers.
  fn pop(&mut self, count: usize) {
    self.ops.extend(repeat_n(SvgOp::Pop, count));
  }

  fn push_node(&mut self, node: &Node) {
    match node {
      Node::Group(group) => self.push_group(group),
      Node::Path(path) => self.push_path(path),
      Node::Image(image) => self.push_image(image),
      Node::Text(text) => self.push_group(text.flattened()),
    }
  }

  fn push_path(&mut self, path: &usvg::Path) {
    if !path.is_visible() {
      return;
    }
    let commands = path_commands(path.data());
    if commands.is_empty() {
      return;
    }

    let anti_alias = path.rendering_mode().anti_alias();
    let fill = path.fill().map(|fill| self.convert_fill(fill));
    let stroke = path.stroke().map(|stroke| self.convert_stroke(stroke));

    if fill.is_none() && stroke.is_none() {
      return;
    }

    match path.paint_order() {
      PaintOrder::FillAndStroke => self.ops.push(SvgOp::Draw {
        path: commands,
        fill,
        stroke,
        anti_alias,
      }),
      PaintOrder::StrokeAndFill => {
        if let Some(stroke) = stroke {
          self.ops.push(SvgOp::Draw {
            path: commands.clone(),
            fill: None,
            stroke: Some(stroke),
            anti_alias,
          });
        }
        if let Some(fill) = fill {
          self.ops.push(SvgOp::Draw {
            path: commands,
            fill: Some(fill),
            stroke: None,
            anti_alias,
          });
        }
      }
    }
  }

  fn push_image(&mut self, image: &usvg::Image) {
    if !image.is_visible() {
      return;
    }

    if let ImageKind::SVG(tree) = image.kind() {
      let size = tree.size();
      self.ops.push(SvgOp::PushClip {
        path: rect_commands(0.0, 0.0, size.width(), size.height()),
        evenodd: false,
      });
      self.push_group(tree.root());
      self.ops.push(SvgOp::Pop);
      return;
    }

    if let Some(data) = image_data(image.kind()) {
      let size = image.size();

      self.ops.push(SvgOp::Image {
        data,
        width: size.width(),
        height: size.height(),
        rendering: scaling_algorithm(image.rendering_mode()),
      });
    }
  }

  /// Emits clip layers for `clip_path`; returns how many layers were pushed.
  ///
  /// Simple clips (no nested clip on a child, uniform nonzero rules or a single
  /// even-odd shape) become native clip ops; anything else becomes an alpha
  /// mask, mirroring krilla-svg.
  fn push_clip_path(&mut self, clip_path: &ClipPath) -> usize {
    let clip_rules = collect_clip_rules(clip_path.root());
    // Uniform nonzero rules always convert; even-odd only as a single shape
    // (overlapping even-odd shapes render differently in PDF).
    let simple = is_simple_clip_path(clip_path.root())
      && match clip_rules.as_slice() {
        [usvg::FillRule::EvenOdd] => true,
        rules => rules.iter().all(|rule| *rule == usvg::FillRule::NonZero),
      };

    if simple {
      let rule = clip_rules
        .first()
        .copied()
        .unwrap_or(usvg::FillRule::NonZero);

      self.push_simple_clips(clip_path, rule)
    } else {
      let op = self.complex_clip_op(clip_path);

      self.ops.push(op);
      1
    }
  }

  fn push_simple_clips(&mut self, clip_path: &ClipPath, rule: usvg::FillRule) -> usize {
    let mut pushed = 0;

    if let Some(nested) = clip_path.clip_path() {
      pushed += self.push_simple_clips(nested, rule);
    }

    let mut commands = Vec::new();

    extend_clip_commands(clip_path.root(), &clip_path.transform(), &mut commands);
    if commands.is_empty() {
      // A clip path with only hidden children still hides everything.
      commands.push(PathCommand::MoveTo(Point::ZERO));
      commands.push(PathCommand::LineTo(Point::ZERO));
    }
    self.ops.push(SvgOp::PushClip {
      path: commands,
      evenodd: rule == usvg::FillRule::EvenOdd,
    });
    pushed + 1
  }

  fn complex_clip_op(&self, clip_path: &ClipPath) -> SvgOp {
    let ops = self.nested(|mask| {
      let mut pop_count = 0;

      if let Some(nested) = clip_path.clip_path() {
        let op = mask.complex_clip_op(nested);

        mask.ops.push(op);
        pop_count += 1;
      }
      if clip_path.transform() != Transform::identity() {
        mask
          .ops
          .push(SvgOp::PushTransform(transform_array(clip_path.transform())));
        pop_count += 1;
      }
      mask.push_group(clip_path.root());
      mask.pop(pop_count);
    });

    SvgOp::PushMask {
      ops,
      luminance: false,
    }
  }

  fn mask_op(&self, mask: &Mask) -> SvgOp {
    let ops = self.nested(|flattener| {
      let mut pop_count = 0;

      if let Some(nested) = mask.mask() {
        let op = flattener.mask_op(nested);

        flattener.ops.push(op);
        pop_count += 1;
      }

      let rect = mask.rect();

      flattener.ops.push(SvgOp::PushClip {
        path: rect_commands(rect.x(), rect.y(), rect.width(), rect.height()),
        evenodd: false,
      });
      pop_count += 1;
      flattener.push_group(mask.root());
      flattener.pop(pop_count);
    });

    SvgOp::PushMask {
      ops,
      luminance: mask.kind() == MaskType::Luminance,
    }
  }

  fn convert_fill(&self, fill: &usvg::Fill) -> SvgFill {
    SvgFill {
      paint: self.convert_paint(fill.paint()),
      opacity: fill.opacity().get(),
      evenodd: fill.rule() == usvg::FillRule::EvenOdd,
    }
  }

  fn convert_stroke(&self, stroke: &usvg::Stroke) -> SvgStrokeStyle {
    SvgStrokeStyle {
      paint: self.convert_paint(stroke.paint()),
      opacity: stroke.opacity().get(),
      width: stroke.width().get(),
      miter_limit: stroke.miterlimit().get(),
      cap: match stroke.linecap() {
        usvg::LineCap::Butt => SvgLineCap::Butt,
        usvg::LineCap::Round => SvgLineCap::Round,
        usvg::LineCap::Square => SvgLineCap::Square,
      },
      join: match stroke.linejoin() {
        usvg::LineJoin::Miter => SvgLineJoin::Miter,
        usvg::LineJoin::MiterClip => SvgLineJoin::MiterClip,
        usvg::LineJoin::Round => SvgLineJoin::Round,
        usvg::LineJoin::Bevel => SvgLineJoin::Bevel,
      },
      dash: stroke
        .dasharray()
        .map(|array| (array.to_vec(), stroke.dashoffset())),
    }
  }

  fn convert_paint(&self, paint: &Paint) -> SvgPaint {
    match paint {
      Paint::Color(color) => SvgPaint::Color([color.red, color.green, color.blue]),
      Paint::LinearGradient(linear) => SvgPaint::Linear {
        start: Point {
          x: linear.x1(),
          y: linear.y1(),
        },
        end: Point {
          x: linear.x2(),
          y: linear.y2(),
        },
        gradient: convert_gradient(linear.transform(), linear.spread_method(), linear.stops()),
      },
      Paint::RadialGradient(radial) => SvgPaint::Radial {
        center: Point {
          x: radial.cx(),
          y: radial.cy(),
        },
        radius: radial.r().get(),
        focal: Point {
          x: radial.fx(),
          y: radial.fy(),
        },
        focal_radius: radial.fr().get(),
        gradient: convert_gradient(radial.transform(), radial.spread_method(), radial.stops()),
      },
      Paint::Pattern(pattern) => {
        let rect = pattern.rect();
        let transform = pattern
          .transform()
          .pre_concat(Transform::from_translate(rect.x(), rect.y()));

        SvgPaint::Pattern {
          ops: self.nested(|tile| tile.push_group(pattern.root())),
          transform: transform_array(transform),
          width: rect.width(),
          height: rect.height(),
        }
      }
    }
  }
}

/// The pixels `kind` carries, or `None` for an SVG or a bitmap that does not decode.
fn image_data(kind: &ImageKind) -> Option<SvgImageData> {
  match kind {
    ImageKind::JPEG(bytes) | ImageKind::PNG(bytes) | ImageKind::WEBP(bytes) => {
      Some(SvgImageData::Encoded(Arc::clone(bytes)))
    }
    #[cfg(feature = "gif")]
    ImageKind::GIF(bytes) => {
      let mut first = None;

      decode_gif_frames(bytes, 0, Some(1), None, |frame| first = Some(frame)).ok()?;
      first.map(|frame| SvgImageData::Bitmap(Arc::unwrap_or_clone(frame)))
    }
    #[cfg(not(feature = "gif"))]
    ImageKind::GIF(_) => None,
    ImageKind::Glyph(glyph) => match glyph.as_ref() {
      ResolvedGlyph::Bitmap(bitmap) => Some(SvgImageData::Bitmap(bitmap.image.clone())),
      ResolvedGlyph::Outline(_) => None,
    },
    ImageKind::SVG(_) => None,
  }
}

const fn scaling_algorithm(rendering: ImageRendering) -> ImageScalingAlgorithm {
  match rendering {
    ImageRendering::OptimizeQuality | ImageRendering::Smooth | ImageRendering::HighQuality => {
      ImageScalingAlgorithm::Auto
    }
    ImageRendering::OptimizeSpeed | ImageRendering::CrispEdges | ImageRendering::Pixelated => {
      ImageScalingAlgorithm::Pixelated
    }
  }
}

/// The group a node's children live in: the node itself for groups, the flattened outlines for
/// text.
fn subgroup(node: &Node) -> Option<&Group> {
  match node {
    Node::Group(group) => Some(group),
    Node::Text(text) => Some(text.flattened()),
    _ => None,
  }
}

fn extend_clip_commands(group: &Group, transform: &Transform, commands: &mut Vec<PathCommand>) {
  for child in group.children() {
    match child {
      Node::Path(path) => {
        if path.is_visible()
          && let Some(transformed) = path.data().clone().transform(*transform)
        {
          commands.extend(path_commands(&transformed));
        }
      }
      node => {
        if let Some(group) = subgroup(node) {
          let group_transform = transform.pre_concat(group.transform());

          extend_clip_commands(group, &group_transform, commands);
        }
      }
    }
  }
}

fn is_simple_clip_path(group: &Group) -> bool {
  group.children().iter().all(|node| match subgroup(node) {
    Some(group) => group.clip_path().is_none() && is_simple_clip_path(group),
    None => true,
  })
}

fn collect_clip_rules(group: &Group) -> Vec<usvg::FillRule> {
  let mut rules = Vec::new();

  for node in group.children() {
    match node {
      Node::Path(path) => {
        if let Some(fill) = path.fill() {
          rules.push(fill.rule());
        }
      }
      node => {
        if let Some(group) = subgroup(node) {
          rules.extend(collect_clip_rules(group));
        }
      }
    }
  }
  rules
}

fn convert_gradient(
  transform: Transform,
  spread: usvg::SpreadMethod,
  stops: &[usvg::Stop],
) -> SvgGradient {
  SvgGradient {
    transform: transform_array(transform),
    spread: match spread {
      usvg::SpreadMethod::Pad => SvgSpreadMethod::Pad,
      usvg::SpreadMethod::Reflect => SvgSpreadMethod::Reflect,
      usvg::SpreadMethod::Repeat => SvgSpreadMethod::Repeat,
    },
    stops: stops
      .iter()
      .map(|stop| SvgGradientStop {
        offset: stop.offset().get(),
        color: [stop.color().red, stop.color().green, stop.color().blue],
        opacity: stop.opacity().get(),
      })
      .collect(),
  }
}

fn convert_blend_mode(blend: usvg::BlendMode) -> BlendMode {
  match blend {
    usvg::BlendMode::Normal => BlendMode::Normal,
    usvg::BlendMode::Multiply => BlendMode::Multiply,
    usvg::BlendMode::Screen => BlendMode::Screen,
    usvg::BlendMode::Overlay => BlendMode::Overlay,
    usvg::BlendMode::Darken => BlendMode::Darken,
    usvg::BlendMode::Lighten => BlendMode::Lighten,
    usvg::BlendMode::ColorDodge => BlendMode::ColorDodge,
    usvg::BlendMode::ColorBurn => BlendMode::ColorBurn,
    usvg::BlendMode::HardLight => BlendMode::HardLight,
    usvg::BlendMode::SoftLight => BlendMode::SoftLight,
    usvg::BlendMode::Difference => BlendMode::Difference,
    usvg::BlendMode::Exclusion => BlendMode::Exclusion,
    usvg::BlendMode::Hue => BlendMode::Hue,
    usvg::BlendMode::Saturation => BlendMode::Saturation,
    usvg::BlendMode::Color => BlendMode::Color,
    usvg::BlendMode::Luminosity => BlendMode::Luminosity,
    usvg::BlendMode::PlusLighter => BlendMode::PlusLighter,
  }
}

fn path_commands(path: &tiny_skia_path::Path) -> Vec<PathCommand> {
  let point = |p: tiny_skia_path::Point| Point { x: p.x, y: p.y };

  path
    .segments()
    .map(|segment| match segment {
      PathSegment::MoveTo(p) => PathCommand::MoveTo(point(p)),
      PathSegment::LineTo(p) => PathCommand::LineTo(point(p)),
      PathSegment::QuadTo(c, p) => PathCommand::QuadTo(point(c), point(p)),
      PathSegment::CubicTo(c1, c2, p) => PathCommand::CubicTo(point(c1), point(c2), point(p)),
      PathSegment::Close => PathCommand::Close,
    })
    .collect()
}

fn rect_commands(x: f32, y: f32, width: f32, height: f32) -> Vec<PathCommand> {
  vec![
    PathCommand::MoveTo(Point { x, y }),
    PathCommand::LineTo(Point { x: x + width, y }),
    PathCommand::LineTo(Point {
      x: x + width,
      y: y + height,
    }),
    PathCommand::LineTo(Point { x, y: y + height }),
    PathCommand::Close,
  ]
}

fn transform_array(transform: Transform) -> [f32; 6] {
  [
    transform.sx,
    transform.ky,
    transform.kx,
    transform.sy,
    transform.tx,
    transform.ty,
  ]
}
