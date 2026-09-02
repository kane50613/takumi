//! Flattens a usvg tree into backend-agnostic vector drawing ops so vector
//! backends (PDF) can embed SVG images as real paths instead of bitmaps.
//! Groups with filters, patterns and embedded raster images fall back to
//! rasterization, mirroring krilla-svg.

use tiny_skia::Pixmap;
use tiny_skia_path::{NonZeroRect, PathSegment, Transform};

use crate::{
  geometry::{PathCommand, Point},
  resvg::{
    render_node,
    usvg::{self, ClipPath, Group, ImageKind, Mask, MaskType, Node, Paint, PaintOrder, Tree},
  },
  style::BlendMode,
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
  },
  /// Draws a pre-rasterized RGBA8 (straight alpha) region: the fallback for subtrees vector ops
  /// cannot express (filters, embedded bitmaps).
  Raster {
    /// Un-premultiplied RGBA8 pixels.
    rgba: Vec<u8>,
    /// Pixel width.
    width: u32,
    /// Pixel height.
    height: u32,
    /// Placement rect `(x, y, w, h)` in the current coordinate space.
    rect: (f32, f32, f32, f32),
  },
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
  /// Sharp corner, subject to the miter limit.
  Miter,
  /// Rounded corner.
  Round,
  /// Beveled corner.
  Bevel,
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
  /// Radial gradient (focal point form, focal radius is always zero).
  Radial {
    /// Center.
    center: Point<f32>,
    /// Radius.
    radius: f32,
    /// Focal point.
    focal: Point<f32>,
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
pub(crate) fn flatten(tree: &Tree, raster_scale: f32) -> Vec<SvgOp> {
  Flattener::new(raster_scale).group(tree.root())
}

/// One op list being written, at one raster scale.
struct Flattener {
  raster_scale: f32,
  ops: Vec<SvgOp>,
}

impl Flattener {
  fn new(raster_scale: f32) -> Self {
    Self {
      raster_scale,
      ops: Vec::new(),
    }
  }

  /// The ops `fill` writes into a fresh list at this scale: a mask, a clip mask or a pattern tile.
  fn nested(&self, fill: impl FnOnce(&mut Self)) -> Vec<SvgOp> {
    let mut nested = Self::new(self.raster_scale);

    fill(&mut nested);
    nested.ops
  }

  fn group(mut self, group: &Group) -> Vec<SvgOp> {
    self.push_group(group);
    self.ops
  }

  fn push_group(&mut self, group: &Group) {
    if !group.filters().is_empty() {
      self.raster_fallback(group);
      return;
    }

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

    for child in group.children() {
      self.push_node(child);
    }

    for _ in 0..pop_count {
      self.ops.push(SvgOp::Pop);
    }
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
      }),
      PaintOrder::StrokeAndFill => {
        if let Some(stroke) = stroke {
          self.ops.push(SvgOp::Draw {
            path: commands.clone(),
            fill: None,
            stroke: Some(stroke),
          });
        }
        if let Some(fill) = fill {
          self.ops.push(SvgOp::Draw {
            path: commands,
            fill: Some(fill),
            stroke: None,
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

    // Encoded bitmap inside the SVG: let resvg draw it (decode, orientation,
    // rendering mode) and embed the pixels.
    let bbox = image.bounding_box();

    self.raster_node(
      &Node::Image(Box::new(image.clone())),
      (bbox.x(), bbox.y(), bbox.width(), bbox.height()),
    );
  }

  /// Rasterizes a group that vector ops cannot express (filters).
  fn raster_fallback(&mut self, group: &Group) {
    let Some(bbox) = group.layer_bounding_box().transform(group.transform()) else {
      return;
    };

    self.raster_node(
      &Node::Group(Box::new(group.clone())),
      (bbox.x(), bbox.y(), bbox.width(), bbox.height()),
    );
  }

  /// Renders `node` through resvg at the raster scale and emits the pixels
  /// placed over `local_bbox`, the node's layer bounding box in the currently
  /// open coordinate space (a group's own transform is not yet pushed when it
  /// falls back, so it is part of the placement).
  fn raster_node(&mut self, node: &Node, local_bbox: (f32, f32, f32, f32)) {
    let (bbox_x, bbox_y, bbox_width, bbox_height) = local_bbox;
    if bbox_width <= 0.0 || bbox_height <= 0.0 {
      return;
    }

    // Cap the fallback bitmap at 5000px on the long edge, like krilla-svg.
    const PIXEL_THRESHOLD: f32 = 5000.0;
    let scale = self
      .raster_scale
      .min(PIXEL_THRESHOLD / bbox_width)
      .min(PIXEL_THRESHOLD / bbox_height);

    let width = (bbox_width * scale).round().max(1.0) as u32;
    let height = (bbox_height * scale).round().max(1.0) as u32;
    let Some(mut pixmap) = Pixmap::new(width, height) else {
      return;
    };

    // `resvg::render_node` pre-translates by the node's absolute layer bbox
    // origin; counter it so the pixmap is filled from the local layer bbox
    // instead (same trick as krilla-svg).
    let abs_bbox = node.abs_layer_bounding_box();
    let initial_transform = Transform::from_scale(scale, scale)
      .pre_concat(Transform::from_translate(-bbox_x, -bbox_y))
      .pre_concat(Transform::from_translate(
        abs_bbox.as_ref().map_or(0.0, NonZeroRect::x),
        abs_bbox.as_ref().map_or(0.0, NonZeroRect::y),
      ));

    render_node(node, initial_transform, &mut pixmap.as_mut());

    let rgba = pixmap
      .pixels()
      .iter()
      .flat_map(|pixel| {
        let demultiplied = pixel.demultiply();
        [
          demultiplied.red(),
          demultiplied.green(),
          demultiplied.blue(),
          demultiplied.alpha(),
        ]
      })
      .collect();

    self.ops.push(SvgOp::Raster {
      rgba,
      width,
      height,
      rect: (bbox_x, bbox_y, bbox_width, bbox_height),
    });
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
      commands.push(PathCommand::MoveTo(Point { x: 0.0, y: 0.0 }));
      commands.push(PathCommand::LineTo(Point { x: 0.0, y: 0.0 }));
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
      for _ in 0..pop_count {
        mask.ops.push(SvgOp::Pop);
      }
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
      for _ in 0..pop_count {
        flattener.ops.push(SvgOp::Pop);
      }
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
        usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => SvgLineJoin::Miter,
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
