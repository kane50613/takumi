//! The outline of a shape and the object it becomes, after Blink's `SVGRectElement::AsPath`,
//! `SVGCircleElement::AsPath`, `SVGEllipseElement::AsPath`, `SVGLineElement::AsPath`,
//! `SVGPolyElement::AsPath`, `SVGPathElement::AsPath`, and `LayoutSVGShape`.

use super::{
  Scope, Styled, SvgLayout,
  object::{Container, ContextSource, Fill, LayoutObject, Paint, Shape, Stroke},
};
use crate::{
  geometry::{PathBuilder, PathCommand, Point, Rect},
  path_data::PathData,
  style::{Affine, Color, Length, PaintPhase, ShapeRendering, Size, VectorEffect, Visibility},
  svg::approx_eq::ApproxEq,
  svg::{element::SvgElementKind, length::LengthAxis},
};

fn is_valid_length(length: f32) -> bool {
  length > 0.0 && length.is_finite()
}

impl LayoutObject {
  /// The background of the root as a shape: a rectangle of `color`.
  pub(super) fn background(canvas: Rect<f32>, color: Color) -> Option<Self> {
    let commands = PathCommand::rectangle(canvas.x(), canvas.y(), canvas.width(), canvas.height());
    let normalized = PathCommand::normalize(&commands)?;

    Some(LayoutObject::Shape(Box::new(Shape {
      commands: normalized.commands,
      bounds: normalized.bounds,
      visible: true,
      fill: Some(Fill {
        paint: Paint::Color(Color([color.0[0], color.0[1], color.0[2], 255])),
        opacity: f32::from(color.0[3]) / 255.0,
        evenodd: false,
        context: None,
      }),
      stroke: None,
      stroke_first: false,
      anti_alias: true,
      abs_transform: Affine::IDENTITY,
    })))
  }
}

impl SvgLayout<'_> {
  /// The outline the element draws, none when its geometry is invalid.
  pub(super) fn shape_commands(&self, styled: &Styled, scope: &Scope) -> Option<Vec<PathCommand>> {
    match styled.kind {
      SvgElementKind::Rect => self.rect_commands(styled, scope),
      SvgElementKind::Circle => self.circle_commands(styled, scope),
      SvgElementKind::Ellipse => self.ellipse_commands(styled, scope),
      SvgElementKind::Line => Some(self.line_commands(styled, scope)),
      SvgElementKind::Polyline => self.points_commands(styled.index, false),
      SvgElementKind::Polygon => self.points_commands(styled.index, true),
      SvgElementKind::Path => {
        let path = &styled.style().svg_geometry_data.d;

        (!path.is_empty()).then(|| path.commands().to_vec())
      }
      _ => None,
    }
  }

  fn size_px(&self, styled: &Styled, scope: &Scope, size: Size, axis: LengthAxis) -> f32 {
    match size {
      Size::Length(length) => self.px(styled, scope, length, axis),
      _ => 0.0,
    }
  }

  fn rect_commands(&self, styled: &Styled, scope: &Scope) -> Option<Vec<PathCommand>> {
    let style = styled.style();
    let width = self.size_px(styled, scope, style.box_data.width, LengthAxis::X);
    let height = self.size_px(styled, scope, style.box_data.height, LengthAxis::Y);

    if !is_valid_length(width) || !is_valid_length(height) {
      return None;
    }

    let geometry = &style.svg_geometry_data;
    let x = self.px(styled, scope, geometry.x, LengthAxis::X);
    let y = self.px(styled, scope, geometry.y, LengthAxis::Y);
    let (mut rx, mut ry) = self.radii(styled, scope);

    // The radii are at most half the size, once resolved.
    if rx > width / 2.0 {
      rx = width / 2.0;
    }

    if ry > height / 2.0 {
      ry = height / 2.0;
    }

    if rx.approx_eq(0.0) {
      Rect::from_xywh(x, y, width, height)?;

      return Some(PathCommand::rectangle(x, y, width, height));
    }

    // https://www.w3.org/TR/SVG11/shapes.html#RectElement
    let (right, bottom) = (x + width, y + height);
    let mut outline = Vec::new();

    outline.move_to((x + rx, y));
    outline.line_to((right - rx, y));
    outline.extend(PathData::corner_arc(
      (right - rx, y),
      (right, y + ry),
      rx,
      ry,
    ));
    outline.line_to((right, bottom - ry));
    outline.extend(PathData::corner_arc(
      (right, bottom - ry),
      (right - rx, bottom),
      rx,
      ry,
    ));
    outline.line_to((x + rx, bottom));
    outline.extend(PathData::corner_arc(
      (x + rx, bottom),
      (x, bottom - ry),
      rx,
      ry,
    ));
    outline.line_to((x, y + ry));
    outline.extend(PathData::corner_arc((x, y + ry), (x + rx, y), rx, ry));
    outline.close();
    Some(outline)
  }

  /// `rx` and `ry` in user units, where a radius that is missing takes the other.
  fn radii(&self, styled: &Styled, scope: &Scope) -> (f32, f32) {
    let geometry = &styled.style().svg_geometry_data;
    let radius = |length: Length, axis| {
      // A `calc()` below zero clamps to zero, as the properties are non-negative.
      (!matches!(length, Length::Auto)).then(|| self.px(styled, scope, length, axis).max(0.0))
    };

    match (
      radius(geometry.rx, LengthAxis::X),
      radius(geometry.ry, LengthAxis::Y),
    ) {
      (None, None) => (0.0, 0.0),
      (Some(rx), None) => (rx, rx),
      (None, Some(ry)) => (ry, ry),
      (Some(rx), Some(ry)) => (rx, ry),
    }
  }

  fn circle_commands(&self, styled: &Styled, scope: &Scope) -> Option<Vec<PathCommand>> {
    let geometry = &styled.style().svg_geometry_data;
    let cx = self.px(styled, scope, geometry.cx, LengthAxis::X);
    let cy = self.px(styled, scope, geometry.cy, LengthAxis::Y);
    let r = self.px(styled, scope, geometry.r, LengthAxis::Other);

    is_valid_length(r).then(|| ellipse_outline(cx, cy, r, r))
  }

  fn ellipse_commands(&self, styled: &Styled, scope: &Scope) -> Option<Vec<PathCommand>> {
    let geometry = &styled.style().svg_geometry_data;
    let cx = self.px(styled, scope, geometry.cx, LengthAxis::X);
    let cy = self.px(styled, scope, geometry.cy, LengthAxis::Y);
    let (rx, ry) = self.radii(styled, scope);

    (is_valid_length(rx) && is_valid_length(ry)).then(|| ellipse_outline(cx, cy, rx, ry))
  }

  fn line_commands(&self, styled: &Styled, scope: &Scope) -> Vec<PathCommand> {
    let coordinate = |name, axis| self.attribute_px(styled, scope, name, axis, Length::zero());

    vec![
      PathCommand::MoveTo(Point::new(
        coordinate("x1", LengthAxis::X),
        coordinate("y1", LengthAxis::Y),
      )),
      PathCommand::LineTo(Point::new(
        coordinate("x2", LengthAxis::X),
        coordinate("y2", LengthAxis::Y),
      )),
    ]
  }

  /// The outline of a `polyline` or a `polygon`, none unless it has two points.
  fn points_commands(&self, index: usize, close: bool) -> Option<Vec<PathCommand>> {
    let points = PathData::parse_points(self.attribute(index, "points")?);

    if points.len() < 2 {
      return None;
    }

    let mut commands: Vec<PathCommand> = points
      .iter()
      .enumerate()
      .map(|(position, point)| {
        if position == 0 {
          PathCommand::MoveTo(*point)
        } else {
          PathCommand::LineTo(*point)
        }
      })
      .collect();

    if close {
      commands.push(PathCommand::Close);
    }

    Some(commands)
  }

  /// Lays a shape out: its fill and stroke, and the markers on it.
  pub(super) fn convert_shape(
    &mut self,
    styled: &Styled,
    scope: &Scope,
    commands: Vec<PathCommand>,
    parent: &mut Container,
  ) {
    let Some(outline) = PathCommand::normalize(&commands) else {
      return;
    };

    if outline.commands.len() < 2 {
      return;
    }

    let style = styled.style();
    let has_bbox = outline.control_bounds.width() > 0.0 && outline.control_bounds.height() > 0.0;
    let mut fill = self.resolve_fill(styled, has_bbox, scope);
    let mut stroke = self.resolve_stroke(styled, has_bbox, scope);
    let visibility = style.inherited_data.visibility == Visibility::Visible;
    let mut visible = visibility;
    let phases = style.svg_inherited_data.paint_order.phases();
    let stroke_first = style.svg_inherited_data.paint_order.strokes_first();

    // A shape with neither fill nor stroke is invisible.
    if fill.is_none() && stroke.is_none() {
      visible = false;
    }

    let path_transform = parent.abs_transform;

    // A paint that takes its geometry from the shape a marker sits on resolves against it now.
    if let Some(fill) = fill.as_mut() {
      Self::settle_marker_context(
        &mut fill.paint,
        &mut fill.context,
        path_transform,
        outline.control_bounds,
      );
    }

    if let Some(stroke) = stroke.as_mut() {
      Self::settle_marker_context(
        &mut stroke.paint,
        &mut stroke.context,
        path_transform,
        outline.control_bounds,
      );
    }

    let mut markers = None;

    if self.has_markers(styled, scope) && visibility {
      let mut marker_scope = scope.clone();
      let bounds = outline.bounds.has_area().then_some(outline.bounds);

      marker_scope.context_paints = Some((
        fill.clone().map(|mut fill| {
          fill.context = Some(ContextSource::PathNode(path_transform, bounds));
          fill
        }),
        stroke.clone().map(|mut stroke| {
          stroke.context = Some(ContextSource::PathNode(path_transform, bounds));
          stroke
        }),
      ));

      let mut group = Container::new(parent.abs_transform);

      self.convert_markers(styled, &outline.commands, &marker_scope, &mut group);
      group.finish();
      markers = Some(group);
    }

    let mut shape = Shape {
      commands: outline.commands,
      bounds: outline.bounds,
      visible,
      fill,
      stroke,
      stroke_first,
      anti_alias: !matches!(
        style.svg_inherited_data.shape_rendering,
        ShapeRendering::CrispEdges | ShapeRendering::OptimizeSpeed
      ),
      abs_transform: path_transform,
    };

    shape.settle_paints(None);

    let non_scaling = style.svg_data.vector_effect == VectorEffect::NonScalingStroke;
    let objects = |shape: Shape| {
      if non_scaling {
        shape.with_non_scaling_stroke()
      } else {
        vec![LayoutObject::Shape(Box::new(shape))]
      }
    };
    let single_phase = |shape: &Shape, phase: PaintPhase| {
      let mut single = shape.clone();

      match phase {
        PaintPhase::Fill if single.fill.is_some() => single.stroke = None,
        PaintPhase::Stroke if single.stroke.is_some() => single.fill = None,
        _ => return Vec::new(),
      }

      objects(single)
    };

    match (phases, markers) {
      ([PaintPhase::Markers, ..], Some(markers)) => {
        parent
          .children
          .push(LayoutObject::Container(Box::new(markers)));
        parent.children.extend(objects(shape));
      }
      ([first, PaintPhase::Markers, last], Some(markers)) => {
        parent.children.extend(single_phase(&shape, first));
        parent
          .children
          .push(LayoutObject::Container(Box::new(markers)));
        parent.children.extend(single_phase(&shape, last));
      }
      ([_, _, PaintPhase::Markers], Some(markers)) => {
        parent.children.extend(objects(shape));
        parent
          .children
          .push(LayoutObject::Container(Box::new(markers)));
      }
      _ => parent.children.extend(objects(shape)),
    }
  }

  /// Places a paint that takes its geometry from the shape a marker sits on, then forgets the
  /// context, whether it could place it or not.
  fn settle_marker_context(
    paint: &mut Paint,
    context: &mut Option<ContextSource>,
    path_transform: Affine,
    bounds: Rect<f32>,
  ) {
    if let Some(ContextSource::PathNode(context_transform, context_bounds)) = *context {
      paint.resolve_geometry(
        Some((context_transform, context_bounds)),
        path_transform,
        bounds,
      );
      *context = None;
    }
  }
}

impl Shape {
  /// Converts the shape's paints to user units, dropping one that cannot be placed.
  pub(super) fn settle_paints(&mut self, use_context: Option<(Affine, Option<Rect<f32>>)>) {
    let path_transform = self.abs_transform;
    let bounds = self.bounds;

    if let Some(mut fill) = self.fill.take()
      && settle(
        &mut fill.paint,
        &mut fill.context,
        use_context,
        path_transform,
        bounds,
      )
    {
      self.fill = Some(fill);
    }

    if let Some(mut stroke) = self.stroke.take() {
      let (stroke_transform, stroke_bounds) = stroke.user_space.unwrap_or((path_transform, bounds));

      if settle(
        &mut stroke.paint,
        &mut stroke.context,
        use_context,
        stroke_transform,
        stroke_bounds,
      ) {
        // A paint settled in the user space of a non-scaling stroke moves into the host's space.
        if stroke.context.is_none()
          && let Some((transform, _)) = stroke.user_space.take()
        {
          stroke.paint = stroke.paint.transformed(transform);
        }

        self.stroke = Some(stroke);
      }
    }
  }
}

impl Shape {
  /// A `vector-effect: non-scaling-stroke` shape, as Blink's `SVGShapePainter` paints it.
  fn with_non_scaling_stroke(self) -> Vec<LayoutObject> {
    let transform = self.abs_transform;
    let Some(stroke) = self.stroke.as_ref() else {
      return vec![LayoutObject::Shape(Box::new(self))];
    };
    // Blink's `SetupNonScalingStrokeContext` paints no stroke where the transform has no inverse.
    let Some(inverse) = transform.normal_inverse() else {
      return self
        .fill
        .is_some()
        .then(|| {
          LayoutObject::Shape(Box::new(Shape {
            stroke: None,
            ..self
          }))
        })
        .into_iter()
        .collect();
    };
    let waiting = matches!(stroke.context, Some(ContextSource::UseNode));
    let stroked = Shape {
      commands: self
        .commands
        .iter()
        .map(|command| command.transformed(transform))
        .collect(),
      bounds: self.bounds.transformed(transform).unwrap_or(self.bounds),
      fill: None,
      stroke: Some(Stroke {
        paint: if waiting {
          stroke.paint.clone()
        } else {
          stroke.paint.transformed(transform)
        },
        user_space: waiting.then_some((transform, self.bounds)),
        ..stroke.clone()
      }),
      abs_transform: Affine::IDENTITY,
      ..self.clone()
    };
    let mut host = Container::new(Affine::IDENTITY);

    host.transform = inverse;
    host.children.push(LayoutObject::Shape(Box::new(stroked)));
    host.finish();

    let stroke = LayoutObject::Container(Box::new(host));
    let fill = self.fill.is_some().then(|| {
      LayoutObject::Shape(Box::new(Shape {
        stroke: None,
        ..self.clone()
      }))
    });

    if self.stroke_first {
      [Some(stroke), fill].into_iter().flatten().collect()
    } else {
      [fill, Some(stroke)].into_iter().flatten().collect()
    }
  }
}

/// Places one paint, false when it cannot be.
fn settle(
  paint: &mut Paint,
  context: &mut Option<ContextSource>,
  use_context: Option<(Affine, Option<Rect<f32>>)>,
  path_transform: Affine,
  bounds: Rect<f32>,
) -> bool {
  let waiting = matches!(context, Some(ContextSource::UseNode));

  if waiting && use_context.is_none() {
    return true;
  }

  let placed = paint.resolve_geometry(
    if waiting { use_context } else { None },
    path_transform,
    bounds,
  );

  if placed {
    *context = None;
  }

  placed
}

/// An ellipse as four arcs, after Blink's `Path::AddEllipse`.
fn ellipse_outline(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<PathCommand> {
  let mut outline = Vec::new();

  outline.move_to((cx + rx, cy));
  outline.extend(PathData::corner_arc((cx + rx, cy), (cx, cy + ry), rx, ry));
  outline.extend(PathData::corner_arc((cx, cy + ry), (cx - rx, cy), rx, ry));
  outline.extend(PathData::corner_arc((cx - rx, cy), (cx, cy - ry), rx, ry));
  outline.extend(PathData::corner_arc((cx, cy - ry), (cx + rx, cy), rx, ry));
  outline.close();
  outline
}
