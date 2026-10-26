//! Markers on the vertices of a shape, after Blink's `SVGMarkerDataBuilder` and
//! `LayoutSVGResourceMarker::MarkerTransformation`.

use std::{
  f32::consts::{FRAC_PI_2, PI},
  rc::Rc,
};

use super::{
  Scope, Styled, SvgLayout,
  object::{Container, LayoutClip, LayoutObject},
  view_box::PreserveAspectRatio,
};
use crate::{
  geometry::{PathCommand, Point, Rect, Size},
  resources::svg_size::ViewBox,
  style::{Affine, Angle, FromCssStr, Length, Overflow, VectorEffect},
  svg::approx_eq::ApproxEq,
  svg::{element::SvgElementKind, length::LengthAxis},
};

/// A path segment with its quadratics raised to cubics.
#[derive(Debug, Clone, Copy)]
enum Segment {
  MoveTo(Point<f32>),
  LineTo(Point<f32>),
  CubicTo(Point<f32>, Point<f32>, Point<f32>),
  Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkerKind {
  Start,
  Middle,
  End,
}

#[derive(Debug, Clone, Copy)]
enum MarkerOrientation {
  Auto,
  AutoStartReverse,
  Angle(f32),
}

impl SvgLayout<'_> {
  /// Whether the shape has markers to lay out: none inside a clip path, and one at least that
  /// names an element.
  pub(super) fn has_markers(&self, styled: &Styled, scope: &Scope) -> bool {
    if scope.parent_clip_path.is_some() {
      return false;
    }

    let data = &styled.style().svg_resources_data;

    [&data.marker_start, &data.marker_mid, &data.marker_end]
      .into_iter()
      .any(|marker| {
        marker
          .as_ref()
          .and_then(|resource| self.reference(resource))
          .is_some()
      })
  }

  /// Lays out the markers of the shape with `commands` into `parent`.
  pub(super) fn convert_markers(
    &mut self,
    styled: &Styled,
    commands: &[PathCommand],
    scope: &Scope,
    parent: &mut Container,
  ) {
    let data = &styled.style().svg_resources_data;
    let markers = [
      (&data.marker_start, MarkerKind::Start),
      (&data.marker_mid, MarkerKind::Middle),
      (&data.marker_end, MarkerKind::End),
    ];

    for (marker, kind) in markers {
      let Some(link) = marker
        .as_ref()
        .and_then(|resource| self.reference(resource))
      else {
        continue;
      };

      if self.document.elements[link].kind != SvgElementKind::Marker {
        continue;
      }

      // A marker may not contain itself.
      if scope.parent_markers.contains(&link) {
        continue;
      }

      self.deeper(|layout| layout.lay_out_marker(styled, commands, link, kind, scope, parent));
    }
  }

  fn lay_out_marker(
    &mut self,
    shape: &Styled,
    commands: &[PathCommand],
    marker: usize,
    kind: MarkerKind,
    scope: &Scope,
    parent: &mut Container,
  ) -> Option<()> {
    let marker_styled = self.styled_in_place(marker);
    let stroke_scale = self.stroke_scale(shape, marker, scope, parent.abs_transform)?;
    let rect = self.marker_rect(&marker_styled, scope)?;
    let view_box = self
      .attribute(marker, "viewBox")
      .and_then(ViewBox::parse)
      .map(|view_box| {
        let aspect = self
          .attribute(marker, "preserveAspectRatio")
          .and_then(PreserveAspectRatio::parse)
          .unwrap_or_default();

        (view_box, aspect)
      });
    let clips = matches!(
      marker_styled.style().base_data.overflow_x,
      Overflow::Hidden | Overflow::Clip
    );
    let clip_path = if clips {
      let clip_rect = match view_box {
        Some((view_box, _)) => {
          Rect::positive_from_xywh(view_box.x, view_box.y, view_box.width, view_box.height)?
        }
        None => Rect::positive_from_xywh(0.0, 0.0, rect.width(), rect.height())?,
      };

      Some(Rc::new(LayoutClip {
        transform: Affine::IDENTITY,
        clip_path: None,
        root: Container::filled_rectangle(clip_rect)?,
      }))
    } else {
      None
    };
    let segments = segments(commands);
    let orientation = self.orientation(marker);

    for (position, index) in vertices(&segments, kind) {
      if !self.budget.spend_instance() {
        break;
      }

      let mut transform = Affine::translation(position.x, position.y);
      let angle = match orientation {
        MarkerOrientation::AutoStartReverse if index == 0 => {
          (vertex_angle(&segments, index) + 180.0) % 360.0
        }
        MarkerOrientation::Auto | MarkerOrientation::AutoStartReverse => {
          vertex_angle(&segments, index)
        }
        MarkerOrientation::Angle(angle) => angle,
      };

      if !angle.approx_eq(0.0) {
        transform *= Affine::rotation(Angle::new(angle));
      }

      if let Some((view_box, aspect)) = view_box {
        let size = Size::new(rect.width() * stroke_scale, rect.height() * stroke_scale);

        if !(size.width > 0.0
          && size.height > 0.0
          && size.width.is_finite()
          && size.height.is_finite())
        {
          return None;
        }

        let view_box_transform = view_box.to_transform(aspect, size);
        let scale_x = (view_box_transform.a * view_box_transform.a
          + view_box_transform.c * view_box_transform.c)
          .sqrt();
        let scale_y = (view_box_transform.b * view_box_transform.b
          + view_box_transform.d * view_box_transform.d)
          .sqrt();

        transform *= Affine::scale(scale_x, scale_y);
      } else {
        transform *= Affine::scale(stroke_scale, stroke_scale);
      }

      transform *= Affine::translation(-rect.x(), -rect.y());

      let mut group = Container::new(parent.abs_transform * transform);

      group.transform = transform;
      group.clip_path = clip_path.clone();

      let mut marker_scope = scope.clone();

      marker_scope.parent_markers.push(marker);
      self.convert_children(marker, &marker_styled.context, &marker_scope, &mut group);
      group.finish();

      if group.has_children() {
        parent
          .children
          .push(LayoutObject::Container(Box::new(group)));
      }
    }

    Some(())
  }

  /// How much the marker scales: its stroke width, or one for `markerUnits="userSpaceOnUse"`. A
  /// non-scaling stroke under `transform` counts in the host's units, as Blink's
  /// `LayoutSVGShape::StrokeWidthForMarkerUnits` measures it.
  fn stroke_scale(
    &self,
    shape: &Styled,
    marker: usize,
    scope: &Scope,
    transform: Affine,
  ) -> Option<f32> {
    if self.attribute(marker, "markerUnits") == Some("userSpaceOnUse") {
      return Some(1.0);
    }

    let mut width = self.px(
      shape,
      scope,
      shape.style().svg_stroke_data.stroke_width,
      LengthAxis::Other,
    );

    if shape.style().svg_data.vector_effect == VectorEffect::NonScalingStroke {
      width /= transform.root_mean_square_scale();
    }

    (width > 0.0 && width.is_finite()).then_some(width)
  }

  /// The marker's reference point and size.
  fn marker_rect(&self, marker: &Styled, scope: &Scope) -> Option<Rect<f32>> {
    let number = |name, axis, default| self.attribute_px(marker, scope, name, axis, default);

    Rect::positive_from_xywh(
      number("refX", LengthAxis::X, Length::zero()),
      number("refY", LengthAxis::Y, Length::zero()),
      number("markerWidth", LengthAxis::X, Length::Px(3.0)),
      number("markerHeight", LengthAxis::Y, Length::Px(3.0)),
    )
  }

  fn orientation(&self, marker: usize) -> MarkerOrientation {
    match self.attribute(marker, "orient") {
      Some("auto") => MarkerOrientation::Auto,
      Some("auto-start-reverse") => MarkerOrientation::AutoStartReverse,
      Some(text) => MarkerOrientation::Angle(Angle::from_css_str(text).map_or(0.0, |angle| *angle)),
      None => MarkerOrientation::Angle(0.0),
    }
  }
}

fn segments(commands: &[PathCommand]) -> Vec<Segment> {
  let mut segments = Vec::with_capacity(commands.len());
  let mut previous = Point::ZERO;
  let mut subpath_start = Point::ZERO;

  for command in commands {
    match *command {
      PathCommand::MoveTo(point) => {
        segments.push(Segment::MoveTo(point));
        previous = point;
        subpath_start = point;
      }
      PathCommand::LineTo(point) => {
        segments.push(Segment::LineTo(point));
        previous = point;
      }
      PathCommand::QuadTo(control, point) => {
        let blend = |first: f32, second: f32| (first + second * 2.0) / 3.0;

        segments.push(Segment::CubicTo(
          Point::new(blend(previous.x, control.x), blend(previous.y, control.y)),
          Point::new(blend(point.x, control.x), blend(point.y, control.y)),
          point,
        ));
        previous = point;
      }
      PathCommand::CubicTo(first, second, point) => {
        segments.push(Segment::CubicTo(first, second, point));
        previous = point;
      }
      PathCommand::Close => {
        segments.push(Segment::Close);
        previous = subpath_start;
      }
    }
  }

  segments
}

/// The vertices a marker of `kind` sits on, with their index in the path.
fn vertices(path: &[Segment], kind: MarkerKind) -> Vec<(Point<f32>, usize)> {
  let mut vertices = Vec::new();

  match kind {
    MarkerKind::Start => {
      if let Some(Segment::MoveTo(point)) = path.first().copied() {
        vertices.push((point, 0));
      }
    }
    MarkerKind::Middle => {
      let total = path.len().saturating_sub(1);

      for (index, segment) in path.iter().enumerate().take(total).skip(1) {
        match *segment {
          Segment::MoveTo(point) | Segment::LineTo(point) | Segment::CubicTo(_, _, point) => {
            vertices.push((point, index));
          }
          Segment::Close => {}
        }
      }
    }
    MarkerKind::End => {
      let Some(index) = path.len().checked_sub(1) else {
        return vertices;
      };

      match path[index] {
        Segment::LineTo(point) | Segment::CubicTo(_, _, point) => vertices.push((point, index)),
        Segment::Close => vertices.push((subpath_start(path, index), index)),
        Segment::MoveTo(_) => {}
      }
    }
  }

  vertices
}

fn vertex_angle(path: &[Segment], index: usize) -> f32 {
  if index == 0 {
    // First segment.
    let (Some(first), Some(second)) = (path.first(), path.get(1)) else {
      return 0.0;
    };

    match (*first, *second) {
      (Segment::MoveTo(start), Segment::LineTo(point)) => line_angle(start, point),
      (Segment::MoveTo(start), Segment::CubicTo(control, _, point)) => {
        if approximately_same(start, control) {
          line_angle(start, point)
        } else {
          line_angle(start, control)
        }
      }
      _ => 0.0,
    }
  } else if index == path.len() - 1 {
    // Last segment.
    match (path[index - 1], path[index]) {
      (_, Segment::MoveTo(_)) => 0.0,
      (_, Segment::LineTo(point)) => line_angle(previous_vertex(path, index), point),
      (_, Segment::CubicTo(first, second, point)) => {
        if approximately_same(second, point) {
          line_angle(first, point)
        } else {
          line_angle(second, point)
        }
      }
      (Segment::LineTo(point), Segment::Close) => line_angle(point, subpath_start(path, index)),
      (Segment::CubicTo(_, second, point), Segment::Close) => {
        let next = subpath_start(path, index);

        curves_angle(previous_vertex(path, index), second, point, next, next)
      }
      (_, Segment::Close) => 0.0,
    }
  } else {
    // Middle segments.
    match (path[index], path[index + 1]) {
      (Segment::MoveTo(start), Segment::LineTo(point)) => line_angle(start, point),
      (Segment::MoveTo(start), Segment::CubicTo(control, _, _)) => line_angle(start, control),
      (Segment::LineTo(first), Segment::LineTo(second)) => {
        let previous = previous_vertex(path, index);

        bisector_angle(previous, first, first, second)
      }
      (
        Segment::CubicTo(_, first_control, first_point),
        Segment::CubicTo(second_control, _, second_point),
      ) => curves_angle(
        previous_vertex(path, index),
        first_control,
        first_point,
        second_control,
        second_point,
      ),
      (Segment::LineTo(line), Segment::CubicTo(control, _, point)) => {
        let previous = previous_vertex(path, index);

        curves_angle(previous, previous, line, control, point)
      }
      (Segment::CubicTo(_, control, point), Segment::LineTo(line)) => {
        curves_angle(previous_vertex(path, index), control, point, line, line)
      }
      (Segment::LineTo(point), Segment::MoveTo(_)) => {
        line_angle(previous_vertex(path, index), point)
      }
      (Segment::CubicTo(_, control, point), Segment::MoveTo(_)) => {
        if approximately_same(point, control) {
          line_angle(previous_vertex(path, index), point)
        } else {
          line_angle(control, point)
        }
      }
      (Segment::LineTo(point), Segment::Close) => {
        let previous = previous_vertex(path, index);
        let next = subpath_start(path, index);

        bisector_angle(previous, point, point, next)
      }
      (_, Segment::Close) => line_angle(previous_vertex(path, index), subpath_start(path, index)),
      (_, Segment::MoveTo(_)) | (Segment::Close, _) => 0.0,
    }
  }
}

fn approximately_same(left: Point<f32>, right: Point<f32>) -> bool {
  left.x.approx_eq(right.x) && left.y.approx_eq(right.y)
}

fn line_angle(from: Point<f32>, to: Point<f32>) -> f32 {
  bisector_angle(from, to, from, to)
}

/// The angle at a vertex between two curves, from the previous vertex and control point to the
/// vertex, then to its next control point and vertex.
fn curves_angle(
  previous: Point<f32>,
  incoming_control: Point<f32>,
  vertex: Point<f32>,
  outgoing_control: Point<f32>,
  next: Point<f32>,
) -> f32 {
  if approximately_same(incoming_control, vertex) {
    bisector_angle(previous, vertex, vertex, outgoing_control)
  } else if approximately_same(vertex, outgoing_control) {
    bisector_angle(incoming_control, vertex, vertex, next)
  } else {
    bisector_angle(incoming_control, vertex, vertex, outgoing_control)
  }
}

/// The bisector, in degrees, of the incoming vector `from_start` to `from_end` and the outgoing one
/// `to_start` to `to_end`.
fn bisector_angle(
  from_start: Point<f32>,
  from_end: Point<f32>,
  to_start: Point<f32>,
  to_end: Point<f32>,
) -> f32 {
  fn normalize(radians: f32) -> f32 {
    let value = radians % (PI * 2.0);

    if value < 0.0 { value + PI * 2.0 } else { value }
  }

  fn vector_angle(x: f32, y: f32) -> f32 {
    let radians = y.atan2(x);

    if radians.is_nan() {
      0.0
    } else {
      normalize(radians)
    }
  }

  let incoming = vector_angle(from_end.x - from_start.x, from_end.y - from_start.y);
  let outgoing = vector_angle(to_end.x - to_start.x, to_end.y - to_start.y);
  let half = (outgoing - incoming) * 0.5;
  let mut bisector = incoming + half;

  if FRAC_PI_2 < half.abs() {
    bisector -= PI;
  }

  normalize(bisector).to_degrees()
}

fn subpath_start(path: &[Segment], index: usize) -> Point<f32> {
  path
    .iter()
    .take(index)
    .rev()
    .find_map(|segment| match segment {
      Segment::MoveTo(point) => Some(*point),
      _ => None,
    })
    .unwrap_or(Point::ZERO)
}

fn previous_vertex(path: &[Segment], index: usize) -> Point<f32> {
  match path[index - 1] {
    Segment::MoveTo(point) | Segment::LineTo(point) | Segment::CubicTo(_, _, point) => point,
    Segment::Close => subpath_start(path, index),
  }
}
