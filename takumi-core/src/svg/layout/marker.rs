use std::sync::Arc;

use tiny_skia::Point;

use super::{IdKind, State, value::Angle};
use crate::{
  svg::tree::{
    ApproxEqUlps, ApproxZeroUlps, ClipPath, Fill, Group, Node, NonZeroPositiveF32, NonZeroRect,
    Path, Size, Transform, ViewBox,
  },
  svg::{Attr, Length, Node as SvgNode, PreserveAspectRatio, Tag},
};

// Similar to `tiny_skia::PathSegment`, but without the `QuadTo`.
#[derive(Copy, Clone, Debug)]
enum Segment {
  MoveTo(Point),
  LineTo(Point),
  CubicTo(Point, Point, Point),
  Close,
}

pub(crate) fn is_valid(node: SvgNode) -> bool {
  // `marker-*` attributes cannot be set on shapes inside a `clipPath`.
  if node.ancestors().any(|n| n.tag() == Some(Tag::ClipPath)) {
    return false;
  }

  let Some(style) = node.style() else {
    return false;
  };

  [&style.marker_start, &style.marker_mid, &style.marker_end]
    .into_iter()
    .any(|reference| node.reference(reference).is_some())
}

pub(crate) fn convert(
  node: SvgNode,
  path: &tiny_skia::Path,
  state: &State,
  cache: &mut super::Cache,
  parent: &mut Group,
) {
  let Some(style) = node.style() else {
    return;
  };
  let list = [
    (&style.marker_start, MarkerKind::Start),
    (&style.marker_mid, MarkerKind::Middle),
    (&style.marker_end, MarkerKind::End),
  ];

  for (reference, kind) in &list {
    let mut marker = None;
    if let Some(link) = node.reference(reference)
      && link.tag() == Some(Tag::Marker)
    {
      marker = Some(link);
    }

    if let Some(marker) = marker {
      // TODO: move to svgtree
      // Check for recursive marker.
      if state.parent_markers.contains(&marker) {
        log::warn!("Recursive marker detected: {}", marker.element_id());
        continue;
      }

      resolve(node, path, marker, *kind, state, cache, parent);
    }
  }
}

#[derive(Clone, Copy)]
enum MarkerKind {
  Start,
  Middle,
  End,
}

enum MarkerOrientation {
  Auto,
  AutoStartReverse,
  Angle(f32),
}

fn resolve(
  shape_node: SvgNode,
  path: &tiny_skia::Path,
  marker_node: SvgNode,
  marker_kind: MarkerKind,
  state: &State,
  cache: &mut super::Cache,
  parent: &mut Group,
) -> Option<()> {
  let stroke_scale = stroke_scale(shape_node, marker_node, state)?.get();

  let r = convert_rect(marker_node, state)?;

  let view_box = marker_node.parse_viewbox().map(|vb| ViewBox {
    rect: vb,
    aspect: marker_node
      .parse::<PreserveAspectRatio>(Attr::PreserveAspectRatio)
      .unwrap_or_default(),
  });

  let has_overflow = marker_node.style()?.overflow.clips();

  let clip_path = if has_overflow {
    let clip_rect = if let Some(vbox) = view_box {
      vbox.rect
    } else {
      r.size().to_non_zero_rect(0.0, 0.0)
    };

    let mut clip_path = ClipPath::empty(cache.gen_id(IdKind::ClipPath));

    let mut path = Path::new_simple(Arc::new(tiny_skia::PathBuilder::from_rect(
      clip_rect.to_rect(),
    )))?;
    path.fill = Some(Fill::default());

    clip_path.root.children.push(Node::Path(Box::new(path)));

    Some(Arc::new(clip_path))
  } else {
    None
  };

  // TODO: avoid allocation
  let mut segments: Vec<Segment> = Vec::with_capacity(path.len());
  let mut prev = Point::zero();
  let mut prev_move = Point::zero();
  for seg in path.segments() {
    match seg {
      tiny_skia::PathSegment::MoveTo(p) => {
        segments.push(Segment::MoveTo(p));
        prev = p;
        prev_move = p;
      }
      tiny_skia::PathSegment::LineTo(p) => {
        segments.push(Segment::LineTo(p));
        prev = p;
      }
      tiny_skia::PathSegment::QuadTo(p1, p) => {
        let (p1, p2, p) = quad_to_curve(prev, p1, p);
        segments.push(Segment::CubicTo(p1, p2, p));
        prev = p;
      }
      tiny_skia::PathSegment::CubicTo(p1, p2, p) => {
        segments.push(Segment::CubicTo(p1, p2, p));
        prev = p;
      }
      tiny_skia::PathSegment::Close => {
        segments.push(Segment::Close);
        prev = prev_move;
      }
    }
  }

  let draw_marker = |p: tiny_skia::Point, idx: usize| {
    let mut ts = Transform::from_translate(p.x, p.y);

    let angle = match convert_orientation(marker_node) {
      MarkerOrientation::AutoStartReverse if idx == 0 => {
        (calc_vertex_angle(&segments, idx) + 180.0) % 360.0
      }
      MarkerOrientation::Auto | MarkerOrientation::AutoStartReverse => {
        calc_vertex_angle(&segments, idx)
      }
      MarkerOrientation::Angle(angle) => angle,
    };

    if !angle.approx_zero_ulps(4) {
      ts = ts.pre_rotate(angle);
    }

    if let Some(vbox) = view_box {
      let Some(size) = Size::from_wh(r.width() * stroke_scale, r.height() * stroke_scale) else {
        return;
      };
      let vbox_ts = vbox.to_transform(size);
      let (sx, sy) = vbox_ts.get_scale();
      ts = ts.pre_scale(sx, sy);
    } else {
      ts = ts.pre_scale(stroke_scale, stroke_scale);
    }

    ts = ts.pre_translate(-r.x(), -r.y());

    // TODO: do not create a group when no clipPath
    let mut g = Group {
      transform: ts,
      abs_transform: parent.abs_transform.pre_concat(ts),
      clip_path: clip_path.clone(),
      ..Group::empty()
    };

    let mut marker_state = state.clone();
    marker_state.parent_markers.push(marker_node);
    super::convert_children(marker_node, &marker_state, cache, &mut g);
    g.calculate_bounding_boxes();

    if g.has_children() {
      parent.children.push(Node::Group(Box::new(g)));
    }
  };

  draw_markers(&segments, marker_kind, draw_marker);

  Some(())
}

fn stroke_scale(
  path_node: SvgNode,
  marker_node: SvgNode,
  state: &State,
) -> Option<NonZeroPositiveF32> {
  match marker_node.parse(Attr::MarkerUnits) {
    Some("userSpaceOnUse") => NonZeroPositiveF32::new(1.0),
    _ => NonZeroPositiveF32::new(super::units::convert_other_length(
      path_node.style()?.stroke_width,
      path_node,
      state,
    )),
  }
}

fn draw_markers<P>(path: &[Segment], kind: MarkerKind, mut draw_marker: P)
where
  P: FnMut(tiny_skia::Point, usize),
{
  match kind {
    MarkerKind::Start => {
      if let Some(Segment::MoveTo(p)) = path.first().cloned() {
        draw_marker(p, 0);
      }
    }
    MarkerKind::Middle => {
      let total = path.len() - 1;
      let mut i = 1;
      while i < total {
        let p = match path[i] {
          Segment::MoveTo(p) => p,
          Segment::LineTo(p) => p,
          Segment::CubicTo(_, _, p) => p,
          _ => {
            i += 1;
            continue;
          }
        };

        draw_marker(p, i);

        i += 1;
      }
    }
    MarkerKind::End => {
      let idx = path.len() - 1;
      match path.last().cloned() {
        Some(Segment::LineTo(p)) => {
          draw_marker(p, idx);
        }
        Some(Segment::CubicTo(_, _, p)) => {
          draw_marker(p, idx);
        }
        Some(Segment::Close) => {
          let p = get_subpath_start(path, idx);
          draw_marker(p, idx);
        }
        _ => {}
      }
    }
  }
}

fn calc_vertex_angle(path: &[Segment], idx: usize) -> f32 {
  if idx == 0 {
    // First segment.

    debug_assert!(path.len() > 1);

    let seg1 = path[0];
    let seg2 = path[1];

    match (seg1, seg2) {
      (Segment::MoveTo(pm), Segment::LineTo(p)) => calc_line_angle(pm, p),
      (Segment::MoveTo(pm), Segment::CubicTo(p1, _, p)) => {
        if pm.x.approx_eq_ulps(&p1.x, 4) && pm.y.approx_eq_ulps(&p1.y, 4) {
          calc_line_angle(pm, p)
        } else {
          calc_line_angle(pm, p1)
        }
      }
      _ => 0.0,
    }
  } else if idx == path.len() - 1 {
    // Last segment.

    let seg1 = path[idx - 1];
    let seg2 = path[idx];

    match (seg1, seg2) {
      (_, Segment::MoveTo(_)) => 0.0, // unreachable
      (_, Segment::LineTo(p)) => {
        let prev = get_prev_vertex(path, idx);
        calc_line_angle(prev, p)
      }
      (_, Segment::CubicTo(p1, p2, p)) => {
        if p2.x.approx_eq_ulps(&p.x, 4) && p2.y.approx_eq_ulps(&p.y, 4) {
          calc_line_angle(p1, p)
        } else {
          calc_line_angle(p2, p)
        }
      }
      (Segment::LineTo(p), Segment::Close) => {
        let next = get_subpath_start(path, idx);
        calc_line_angle(p, next)
      }
      (Segment::CubicTo(_, p2, p), Segment::Close) => {
        let prev = get_prev_vertex(path, idx);
        let next = get_subpath_start(path, idx);
        calc_curves_angle(prev, p2, p, next, next)
      }
      (_, Segment::Close) => 0.0,
    }
  } else {
    // Middle segments.

    let seg1 = path[idx];
    let seg2 = path[idx + 1];

    // TODO: Not sure if there is a better way.
    match (seg1, seg2) {
      (Segment::MoveTo(pm), Segment::LineTo(p)) => calc_line_angle(pm, p),
      (Segment::MoveTo(pm), Segment::CubicTo(p1, _, _)) => calc_line_angle(pm, p1),
      (Segment::LineTo(p1), Segment::LineTo(p2)) => {
        let prev = get_prev_vertex(path, idx);
        calc_angle(prev, p1, p1, p2)
      }
      (Segment::CubicTo(_, c1_p2, c1_p), Segment::CubicTo(c2_p1, _, c2_p)) => {
        let prev = get_prev_vertex(path, idx);
        calc_curves_angle(prev, c1_p2, c1_p, c2_p1, c2_p)
      }
      (Segment::LineTo(pl), Segment::CubicTo(p1, _, p)) => {
        let prev = get_prev_vertex(path, idx);
        calc_curves_angle(prev, prev, pl, p1, p)
      }
      (Segment::CubicTo(_, p2, p), Segment::LineTo(pl)) => {
        let prev = get_prev_vertex(path, idx);
        calc_curves_angle(prev, p2, p, pl, pl)
      }
      (Segment::LineTo(p), Segment::MoveTo(_)) => {
        let prev = get_prev_vertex(path, idx);
        calc_line_angle(prev, p)
      }
      (Segment::CubicTo(_, p2, p), Segment::MoveTo(_)) => {
        if p.x.approx_eq_ulps(&p2.x, 4) && p.y.approx_eq_ulps(&p2.y, 4) {
          let prev = get_prev_vertex(path, idx);
          calc_line_angle(prev, p)
        } else {
          calc_line_angle(p2, p)
        }
      }
      (Segment::LineTo(p), Segment::Close) => {
        let prev = get_prev_vertex(path, idx);
        let next = get_subpath_start(path, idx);
        calc_angle(prev, p, p, next)
      }
      (_, Segment::Close) => {
        let prev = get_prev_vertex(path, idx);
        let next = get_subpath_start(path, idx);
        calc_line_angle(prev, next)
      }
      (_, Segment::MoveTo(_)) | (Segment::Close, _) => 0.0,
    }
  }
}

fn calc_line_angle(from: Point, to: Point) -> f32 {
  calc_angle(from, to, from, to)
}

fn calc_curves_angle(
  previous: Point,
  previous_control: Point,
  vertex: Point,
  next_control: Point,
  next: Point,
) -> f32 {
  if previous_control.x.approx_eq_ulps(&vertex.x, 4)
    && previous_control.y.approx_eq_ulps(&vertex.y, 4)
  {
    calc_angle(previous, vertex, vertex, next_control)
  } else if vertex.x.approx_eq_ulps(&next_control.x, 4)
    && vertex.y.approx_eq_ulps(&next_control.y, 4)
  {
    calc_angle(previous_control, vertex, vertex, next)
  } else {
    calc_angle(previous_control, vertex, vertex, next_control)
  }
}

fn calc_angle(in_from: Point, in_to: Point, out_from: Point, out_to: Point) -> f32 {
  use std::f32::consts::*;

  fn normalize(rad: f32) -> f32 {
    let v = rad % (PI * 2.0);
    if v < 0.0 { v + PI * 2.0 } else { v }
  }

  fn vector_angle(vx: f32, vy: f32) -> f32 {
    let rad = vy.atan2(vx);
    if rad.is_nan() { 0.0 } else { normalize(rad) }
  }

  let in_a = vector_angle(in_to.x - in_from.x, in_to.y - in_from.y);
  let out_a = vector_angle(out_to.x - out_from.x, out_to.y - out_from.y);
  let d = (out_a - in_a) * 0.5;

  let mut angle = in_a + d;
  if FRAC_PI_2 < d.abs() {
    angle -= PI;
  }

  normalize(angle).to_degrees()
}

fn get_subpath_start(segments: &[Segment], idx: usize) -> tiny_skia::Point {
  let offset = segments.len() - idx;
  for seg in segments.iter().rev().skip(offset) {
    if let Segment::MoveTo(p) = *seg {
      return p;
    }
  }

  tiny_skia::Point::zero()
}

fn get_prev_vertex(segments: &[Segment], idx: usize) -> tiny_skia::Point {
  match segments[idx - 1] {
    Segment::MoveTo(p) => p,
    Segment::LineTo(p) => p,
    Segment::CubicTo(_, _, p) => p,
    Segment::Close => get_subpath_start(segments, idx),
  }
}

fn convert_rect(node: SvgNode, state: &State) -> Option<NonZeroRect> {
  NonZeroRect::from_xywh(
    node.convert_user_length(Attr::RefX, state, Length::number(0.0)),
    node.convert_user_length(Attr::RefY, state, Length::number(0.0)),
    node.convert_user_length(Attr::MarkerWidth, state, Length::number(3.0)),
    node.convert_user_length(Attr::MarkerHeight, state, Length::number(3.0)),
  )
}

fn convert_orientation(node: SvgNode) -> MarkerOrientation {
  match node.parse(Attr::Orient) {
    Some("auto") => MarkerOrientation::Auto,
    Some("auto-start-reverse") => MarkerOrientation::AutoStartReverse,
    _ => MarkerOrientation::Angle(
      node
        .parse::<Angle>(Attr::Orient)
        .map_or(0.0, |angle| angle.0),
    ),
  }
}

fn quad_to_curve(prev: Point, p1: Point, p: Point) -> (Point, Point, Point) {
  #[inline]
  fn calc(n1: f32, n2: f32) -> f32 {
    (n1 + n2 * 2.0) / 3.0
  }

  (
    Point::from_xy(calc(prev.x, p1.x), calc(prev.y, p1.y)),
    Point::from_xy(calc(p.x, p1.x), calc(p.y, p1.y)),
    p,
  )
}
