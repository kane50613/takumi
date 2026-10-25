//! The objects an SVG document lays out to, after Blink's `LayoutSVGContainer`,
//! `LayoutSVGShape`, and `LayoutSVGImage`: containers and shapes with their transforms, clips,
//! masks, and resolved paint.

use std::rc::Rc;
#[cfg(feature = "svg-raster")]
use std::sync::Arc;

#[cfg(feature = "svg-raster")]
use crate::svg::filter::SvgFilter;
use crate::{
  geometry::{PathCommand, Point, Rect, Size},
  resources::{
    image::{
      SvgGradient, SvgGradientStop, SvgImageData, SvgLineCap, SvgLineJoin, SvgOp, SvgOpPaint,
      SvgSpreadMethod,
    },
    svg_size::ViewBox,
  },
  style::{Affine, BlendMode, Color, ImageScalingAlgorithm},
};

use super::view_box::PreserveAspectRatio;

/// What a length or a coordinate system is relative to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Units {
  UserSpaceOnUse,
  ObjectBoundingBox,
}

/// One object of the laid out tree.
#[derive(Clone)]
pub(super) enum LayoutObject {
  Container(Box<Container>),
  Shape(Box<Shape>),
  Picture(Box<Picture>),
}

/// Blink's `LayoutSVGContainer`: objects drawn under one transform, clip, mask, and opacity.
#[derive(Clone)]
pub(super) struct Container {
  /// The transform from the container's space to its parent's.
  pub(super) transform: Affine,
  /// The transform from the container's space to the canvas.
  pub(super) abs_transform: Affine,
  pub(super) opacity: f32,
  pub(super) blend_mode: BlendMode,
  pub(super) isolate: bool,
  pub(super) clip_path: Option<Rc<LayoutClip>>,
  pub(super) mask: Option<Rc<LayoutMask>>,
  #[cfg(feature = "svg-raster")]
  pub(super) filters: Vec<Arc<SvgFilter>>,
  /// Whether `context-fill` and `context-stroke` below it read this container's paint.
  pub(super) context_element: bool,
  pub(super) children: Vec<LayoutObject>,
  bounding_box: Rect<f32>,
}

impl Container {
  pub(super) fn new(abs_transform: Affine) -> Self {
    Self {
      transform: Affine::IDENTITY,
      abs_transform,
      opacity: 1.0,
      blend_mode: BlendMode::Normal,
      isolate: false,
      clip_path: None,
      mask: None,
      #[cfg(feature = "svg-raster")]
      filters: Vec::new(),
      context_element: false,
      children: Vec::new(),
      bounding_box: Rect::ZERO,
    }
  }

  pub(super) fn has_children(&self) -> bool {
    !self.children.is_empty()
  }

  /// The union of the children's bounds, in the container's own space.
  fn union_of_children(&self) -> Option<Rect<f32>> {
    self
      .children
      .iter()
      .map(LayoutObject::bounds_in_parent)
      .reduce(|union, bounds| union.union(&bounds))
  }

  /// Blink's `ObjectBoundingBox` of the children, none when it has no area.
  pub(super) fn object_bounding_box(&self) -> Option<Rect<f32>> {
    self.union_of_children().filter(Rect::has_area)
  }

  /// A container that fills `rect` with black, the root of the clip path of a viewport.
  pub(super) fn filled_rectangle(rect: Rect<f32>) -> Option<Self> {
    let outline = PathCommand::normalize(&PathCommand::rectangle(
      rect.x(),
      rect.y(),
      rect.width(),
      rect.height(),
    ))?;
    let mut root = Self::new(Affine::IDENTITY);

    root.children.push(LayoutObject::Shape(Box::new(Shape {
      commands: outline.commands,
      bounds: outline.bounds,
      visible: true,
      fill: Some(Fill {
        paint: Paint::Color(Color::black()),
        opacity: 1.0,
        evenodd: false,
        context: None,
      }),
      stroke: None,
      stroke_first: false,
      anti_alias: true,
      abs_transform: Affine::IDENTITY,
    })));
    root.finish();
    Some(root)
  }

  /// Places the paints below that take their geometry from this container, a `use` element.
  pub(super) fn settle_use_paints(&mut self) {
    let context = (self.abs_transform, Some(self.bounding_box));

    self.settle_children(context);
  }

  fn settle_children(&mut self, context: (Affine, Option<Rect<f32>>)) {
    for child in &mut self.children {
      match child {
        LayoutObject::Shape(shape) => shape.settle_paints(Some(context)),
        LayoutObject::Container(container) => container.settle_children(context),
        LayoutObject::Picture(_) => {}
      }
    }
  }

  /// The bounds of the children on the canvas, none for an empty container.
  pub(super) fn absolute_bounds(&self) -> Option<Rect<f32>> {
    self
      .children
      .iter()
      .filter_map(|child| match child {
        LayoutObject::Container(container) => container.absolute_bounds(),
        LayoutObject::Shape(shape) => shape.bounds.transformed(shape.abs_transform),
        LayoutObject::Picture(_) => None,
      })
      .reduce(|union, bounds| union.union(&bounds))
  }

  /// The container as the only child of a new one that carries `transform`.
  pub(super) fn wrapped(mut self, transform: Affine) -> Self {
    self.transform = transform;
    self.abs_transform = transform;

    let mut root = Self::new(Affine::IDENTITY);

    root.children.push(LayoutObject::Container(Box::new(self)));
    root.finish();
    root
  }

  /// Fixes the bounds of the children once they are all in.
  pub(super) fn finish(&mut self) {
    if let Some(bounds) = self.union_of_children() {
      self.bounding_box = bounds;
    }
  }
}

impl LayoutObject {
  /// The object's bounds in its parent's space.
  fn bounds_in_parent(&self) -> Rect<f32> {
    match self {
      Self::Container(container) => container
        .bounding_box
        .transformed(container.transform)
        .unwrap_or(container.bounding_box),
      Self::Shape(shape) => shape.bounds,
      Self::Picture(picture) => picture.bounds(),
    }
  }
}

/// The paint a shape fills or strokes with.
#[derive(Clone)]
pub(super) enum Paint {
  Color(Color),
  Gradient(Rc<GradientPaint>),
  Pattern(Rc<PatternPaint>),
}

impl Paint {
  /// The units its geometry is in.
  pub(super) fn units(&self) -> Units {
    match self {
      Self::Color(_) => Units::UserSpaceOnUse,
      Self::Gradient(gradient) => gradient.units,
      Self::Pattern(pattern) => pattern.units,
    }
  }

  /// The units its content is in.
  pub(super) fn content_units(&self) -> Units {
    match self {
      Self::Pattern(pattern) => pattern.content_units,
      _ => Units::UserSpaceOnUse,
    }
  }

  /// Makes the paint drawable on the canvas, after usvg's `process_paint`: a server in bounding
  /// box units is converted to user units against `bounds`, or against the bounds of the context
  /// element when the paint takes its geometry from one, and then follows the context element's
  /// transform. False when it cannot.
  pub(super) fn resolve_geometry(
    &mut self,
    context: Option<(Affine, Option<Rect<f32>>)>,
    path_transform: Affine,
    bounds: Rect<f32>,
  ) -> bool {
    if self.units() == Units::ObjectBoundingBox || self.content_units() == Units::ObjectBoundingBox
    {
      let bounds = match context {
        Some((_, context_bounds)) => match context_bounds {
          Some(context_bounds) => context_bounds,
          None => return false,
        },
        None => bounds,
      };

      match self.in_user_units(bounds) {
        Some(converted) => *self = converted,
        None => return false,
      }
    }

    if let Some((context_transform, _)) = context {
      self.follow_context(context_transform, path_transform);
    }

    true
  }

  /// The paint with its bounding box units replaced by user units against `bounds`.
  fn in_user_units(&self, bounds: Rect<f32>) -> Option<Self> {
    if !bounds.has_area() {
      return None;
    }

    match self {
      Self::Color(_) => Some(self.clone()),
      Self::Gradient(gradient) => Some(Self::Gradient(Rc::new(GradientPaint {
        geometry: gradient.geometry,
        units: Units::UserSpaceOnUse,
        transform: bounds.unit_square_transform() * gradient.transform,
        spread: gradient.spread,
        stops: gradient.stops.clone(),
      }))),
      Self::Pattern(pattern) => {
        let rect = if pattern.units == Units::ObjectBoundingBox {
          pattern.rect.in_bounding_box(&bounds)?
        } else {
          pattern.rect
        };
        let mut root = pattern.root.clone();

        if pattern.content_units == Units::ObjectBoundingBox && pattern.view_box.is_none() {
          root = root.wrapped(Affine::scale(bounds.width(), bounds.height()));
        }

        if let Some((view_box, aspect)) = pattern.view_box {
          root =
            root.wrapped(view_box.to_transform(aspect, Size::new(rect.width(), rect.height())));
        }

        Some(Self::Pattern(Rc::new(PatternPaint {
          units: Units::UserSpaceOnUse,
          content_units: Units::UserSpaceOnUse,
          transform: pattern.transform,
          rect,
          view_box: pattern.view_box,
          root,
        })))
      }
    }
  }

  /// Shifts the paint by the transform between the context element and the shape it paints, so
  /// that it lands as it does on the context element.
  fn follow_context(&mut self, context_transform: Affine, path_transform: Affine) {
    let Some(reverse) = context_transform
      .invert()
      .and_then(|inverse| (inverse * path_transform).invert())
    else {
      return;
    };

    *self = self.transformed(reverse);
  }

  /// The paint drawn in a space that `transform` maps its user space into.
  pub(super) fn transformed(&self, transform: Affine) -> Self {
    match self {
      Self::Color(_) => self.clone(),
      Self::Gradient(gradient) => Self::Gradient(Rc::new(GradientPaint {
        geometry: gradient.geometry,
        units: gradient.units,
        transform: transform * gradient.transform,
        spread: gradient.spread,
        stops: gradient.stops.clone(),
      })),
      Self::Pattern(pattern) => {
        let mut moved = (**pattern).clone();

        moved.transform = transform * pattern.transform;
        Self::Pattern(Rc::new(moved))
      }
    }
  }

  /// The paint on the canvas as the ops draw it, with a pattern's tile flattened by `tile_ops`.
  pub(super) fn to_op(&self, tile_ops: impl FnOnce(&Container) -> Vec<SvgOp>) -> SvgOpPaint {
    match self {
      Self::Color(color) => SvgOpPaint::Color(*color),
      Self::Gradient(gradient) => gradient.to_op(),
      Self::Pattern(pattern) => SvgOpPaint::Pattern {
        ops: tile_ops(&pattern.root),
        transform: pattern.transform * Affine::translation(pattern.rect.x(), pattern.rect.y()),
        width: pattern.rect.width(),
        height: pattern.rect.height(),
      },
    }
  }
}

/// A linear or radial gradient server.
#[derive(Clone)]
pub(super) struct GradientPaint {
  pub(super) geometry: GradientGeometry,
  pub(super) units: Units,
  pub(super) transform: Affine,
  pub(super) spread: SvgSpreadMethod,
  pub(super) stops: Vec<SvgGradientStop>,
}

#[derive(Clone, Copy)]
pub(super) enum GradientGeometry {
  Linear {
    start: Point<f32>,
    end: Point<f32>,
  },
  Radial {
    center: Point<f32>,
    radius: f32,
    focal: Point<f32>,
    focal_radius: f32,
  },
}

impl GradientPaint {
  fn to_op(&self) -> SvgOpPaint {
    let gradient = SvgGradient {
      transform: self.transform,
      spread: self.spread,
      stops: self.stops.clone(),
    };

    match self.geometry {
      GradientGeometry::Linear { start, end } => SvgOpPaint::Linear {
        start,
        end,
        gradient,
      },
      GradientGeometry::Radial {
        center,
        radius,
        focal,
        focal_radius,
      } => SvgOpPaint::Radial {
        center,
        radius,
        focal,
        focal_radius,
        gradient,
      },
    }
  }
}

/// A pattern server: one tile of content placed on a grid.
#[derive(Clone)]
pub(super) struct PatternPaint {
  pub(super) units: Units,
  pub(super) content_units: Units,
  pub(super) transform: Affine,
  pub(super) rect: Rect<f32>,
  pub(super) view_box: Option<(ViewBox, PreserveAspectRatio)>,
  pub(super) root: Container,
}

/// What `context-fill` and `context-stroke` take their geometry from.
#[derive(Clone, Copy)]
pub(super) enum ContextSource {
  /// The `use` element, whose container is known once its children are laid out.
  UseNode,
  /// A shape a marker is placed on, by its transform and bounds.
  PathNode(Affine, Option<Rect<f32>>),
}

#[derive(Clone)]
pub(super) struct Fill {
  pub(super) paint: Paint,
  pub(super) opacity: f32,
  pub(super) evenodd: bool,
  pub(super) context: Option<ContextSource>,
}

#[derive(Clone)]
pub(super) struct Stroke {
  pub(super) paint: Paint,
  pub(super) opacity: f32,
  pub(super) width: f32,
  pub(super) miter_limit: f32,
  pub(super) cap: SvgLineCap,
  pub(super) join: SvgLineJoin,
  pub(super) dash: Option<Rc<(Vec<f32>, f32)>>,
  pub(super) context: Option<ContextSource>,
  /// The transform and bounds of the user space a non-scaling stroke's paint settles in, while it
  /// waits for a `use` element to give it geometry; the stroke draws in the host's space.
  pub(super) user_space: Option<(Affine, Rect<f32>)>,
}

/// Blink's `LayoutSVGShape`: an outline with the fill and stroke that paint it.
#[derive(Clone)]
pub(super) struct Shape {
  pub(super) commands: Vec<PathCommand>,
  /// The tight bounds of the outline.
  pub(super) bounds: Rect<f32>,
  pub(super) visible: bool,
  pub(super) fill: Option<Fill>,
  pub(super) stroke: Option<Stroke>,
  pub(super) stroke_first: bool,
  pub(super) anti_alias: bool,
  /// The transform from the shape's space to the canvas.
  pub(super) abs_transform: Affine,
}

/// Blink's `LayoutSVGImage`: a bitmap or a nested SVG over its natural size.
#[derive(Clone)]
pub(super) struct Picture {
  pub(super) content: PictureContent,
  pub(super) width: f32,
  pub(super) height: f32,
  pub(super) visible: bool,
  pub(super) rendering: ImageScalingAlgorithm,
}

#[derive(Clone)]
pub(super) enum PictureContent {
  Bitmap(Option<Rc<SvgImageData>>),
  Svg(Rc<LayoutDocument>),
}

impl Picture {
  fn bounds(&self) -> Rect<f32> {
    Rect {
      left: 0.0,
      right: self.width,
      top: 0.0,
      bottom: self.height,
    }
  }
}

/// A laid out SVG document: its canvas size and what it draws.
pub(super) struct LayoutDocument {
  pub(super) width: f32,
  pub(super) height: f32,
  pub(super) root: Container,
}

/// Blink's `LayoutSVGResourceClipper`.
pub(super) struct LayoutClip {
  pub(super) transform: Affine,
  pub(super) clip_path: Option<Rc<LayoutClip>>,
  pub(super) root: Container,
}

/// Blink's `LayoutSVGResourceMasker`.
pub(super) struct LayoutMask {
  pub(super) rect: Rect<f32>,
  pub(super) luminance: bool,
  pub(super) mask: Option<Rc<LayoutMask>>,
  pub(super) root: Container,
}
