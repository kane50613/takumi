use super::conic::{CONIC_TOLERANCE, Conic};
use super::{Path, PathBuilder, RectContour, Stroke, StrokeDash};
use crate::geometry::{Point, Rect};
use crate::style::{Affine, LineCap, LineJoin};

fn line(from: (f32, f32), to: (f32, f32)) -> Path {
  let mut builder = PathBuilder::new();

  builder.move_to(from.0, from.1);
  builder.line_to(to.0, to.1);
  builder.finish().unwrap()
}

fn bounds(path: &Path) -> (f32, f32, f32, f32) {
  let bounds = path.compute_tight_bounds().unwrap();

  (bounds.left, bounds.top, bounds.right, bounds.bottom)
}

#[test]
fn a_butt_capped_line_strokes_to_its_width() {
  let stroke = Stroke {
    width: 4.0,
    ..Stroke::default()
  };
  let outline = line((10.0, 10.0), (30.0, 10.0))
    .stroke(&stroke, 1.0)
    .unwrap();

  assert_eq!(bounds(&outline), (10.0, 8.0, 30.0, 12.0));
}

#[test]
fn square_and_round_caps_reach_half_the_width_past_the_ends() {
  for line_cap in [LineCap::Square, LineCap::Round] {
    let stroke = Stroke {
      width: 4.0,
      line_cap,
      ..Stroke::default()
    };
    let (left, top, right, bottom) = bounds(
      &line((10.0, 10.0), (30.0, 10.0))
        .stroke(&stroke, 1.0)
        .unwrap(),
    );

    assert!(
      (left - 8.0).abs() < 1e-3 && (right - 32.0).abs() < 1e-3,
      "{line_cap:?}"
    );
    assert!(
      (top - 8.0).abs() < 1e-3 && (bottom - 12.0).abs() < 1e-3,
      "{line_cap:?}"
    );
  }
}

#[test]
fn a_right_angle_miters_to_its_corner() {
  let mut builder = PathBuilder::new();

  builder.move_to(10.0, 10.0);
  builder.line_to(30.0, 10.0);
  builder.line_to(30.0, 30.0);

  let stroke = Stroke {
    width: 4.0,
    line_join: LineJoin::Miter,
    ..Stroke::default()
  };
  let outline = builder.finish().unwrap().stroke(&stroke, 1.0).unwrap();

  assert_eq!(bounds(&outline), (10.0, 8.0, 32.0, 30.0));
  assert!(outline.points().contains(&Point::new(32.0, 8.0)));
}

#[test]
fn a_dash_cuts_a_line_into_its_on_intervals() {
  let dash = StrokeDash::new(vec![4.0, 6.0], 0.0).unwrap();
  let dashed = line((0.0, 0.0), (30.0, 0.0)).dash(&dash, 1.0).unwrap();
  let moves = dashed
    .verbs()
    .iter()
    .filter(|&&verb| verb == super::PathVerb::Move)
    .count();

  assert_eq!(moves, 3);
  assert_eq!(
    dashed.bounds(),
    Rect::from_ltrb(0.0, 0.0, 24.0, 0.0).unwrap()
  );
}

#[test]
fn a_path_past_the_dash_limit_strokes_undashed() {
  let line = line((0.0, 0.0), (3_000_000.0, 0.0));
  let stroke = Stroke {
    width: 2.0,
    dash: StrokeDash::new(vec![1.0, 1.0], 0.0),
    ..Stroke::default()
  };

  assert!(line.dash(stroke.dash.as_ref().unwrap(), 1.0).is_none());
  assert_eq!(
    line.stroke(&stroke, 1.0).unwrap().bounds(),
    Rect::from_ltrb(0.0, -1.0, 3_000_000.0, 1.0).unwrap(),
  );
}

#[test]
fn an_invalid_dash_pattern_is_rejected() {
  assert!(StrokeDash::new(vec![4.0], 0.0).is_none());
  assert!(StrokeDash::new(vec![4.0, -1.0], 0.0).is_none());
  assert!(StrokeDash::new(vec![0.0, 0.0], 0.0).is_none());
}

#[test]
fn a_lone_zero_length_dash_draws_its_round_cap() {
  let stroke = Stroke {
    width: 4.0,
    line_cap: LineCap::Round,
    dash: StrokeDash::new(vec![5.0, 5.0], 0.0),
    ..Stroke::default()
  };
  let (left, top, right, bottom) = bounds(
    &line((10.0, 10.0), (10.0, 10.0))
      .stroke(&stroke, 1.0)
      .unwrap(),
  );

  assert!((left - 8.0).abs() < 1e-2 && (right - 12.0).abs() < 1e-2);
  assert!((top - 8.0).abs() < 1e-2 && (bottom - 12.0).abs() < 1e-2);
}

#[test]
fn a_lone_zero_length_line_in_a_gap_dashes_to_nothing() {
  let dash = StrokeDash::new(vec![5.0, 5.0], 5.0).unwrap();

  assert!(line((10.0, 10.0), (10.0, 10.0)).dash(&dash, 1.0).is_none());
}

#[test]
fn a_round_cap_splits_finer_at_a_larger_resolution_scale() {
  let stroke = Stroke {
    width: 4.0,
    line_cap: LineCap::Round,
    ..Stroke::default()
  };
  let verbs = |scale: f32| {
    line((10.0, 10.0), (30.0, 10.0))
      .stroke(&stroke, scale)
      .unwrap()
      .verbs()
      .len()
  };

  assert!(verbs(100.0) > verbs(1.0));
}

#[test]
fn appending_a_builder_replaces_a_pending_move_and_continues_its_contour() {
  let mut builder = PathBuilder::new();
  let mut other = PathBuilder::new();

  builder.move_to(0.0, 0.0);
  builder.line_to(10.0, 0.0);
  builder.move_to(50.0, 50.0);
  other.move_to(20.0, 0.0);
  other.line_to(30.0, 0.0);
  other.close();
  builder.push_builder(&other);
  builder.line_to(25.0, 10.0);

  let path = builder.finish().unwrap();
  let bounds = path.compute_tight_bounds().unwrap();

  assert_eq!((bounds.right, bounds.bottom), (30.0, 10.0));
  assert_eq!(path.points().last().copied(), Some(Point::new(25.0, 10.0)));
  assert_eq!(
    path.points()[path.points().len() - 2],
    Point::new(20.0, 0.0)
  );
}

#[test]
fn a_rect_traced_through_collinear_points_and_an_implicit_close_is_a_rect() {
  let contour = |points: &[(f32, f32)], close: bool| {
    let mut builder = PathBuilder::new();

    builder.move_to(points[0].0, points[0].1);

    for &(x, y) in &points[1..] {
      builder.line_to(x, y);
    }

    if close {
      builder.close();
    }

    builder.finish().unwrap().rect_contour()
  };
  let rect = Rect::from_ltrb(0.0, 0.0, 10.0, 8.0).unwrap();

  assert_eq!(
    contour(
      &[(0.0, 0.0), (4.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)],
      true
    ),
    Some(RectContour {
      rect,
      closed: true,
      clockwise: true,
    })
  );
  assert_eq!(
    contour(
      &[(0.0, 0.0), (0.0, 8.0), (0.0, 8.0), (10.0, 8.0), (10.0, 0.0)],
      false
    ),
    Some(RectContour {
      rect,
      closed: false,
      clockwise: false,
    })
  );
  assert_eq!(
    contour(&[(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (1.0, 8.0)], true),
    None
  );
  assert_eq!(
    contour(&[(0.0, 0.0), (10.0, 0.0), (0.0, 0.0), (10.0, 0.0)], true),
    None
  );
}

#[test]
fn a_closed_rect_strokes_to_its_outer_and_inner_rects() {
  let rect = Path::from_rect(Rect::from_ltrb(10.0, 10.0, 30.0, 20.0).unwrap());
  let outline = |width: f32, line_join: LineJoin| {
    rect
      .stroke(
        &Stroke {
          width,
          line_join,
          ..Stroke::default()
        },
        1.0,
      )
      .unwrap()
  };
  let mitered = outline(4.0, LineJoin::Miter);

  assert_eq!(
    mitered.points(),
    [
      (8.0, 8.0),
      (32.0, 8.0),
      (32.0, 22.0),
      (8.0, 22.0),
      (12.0, 12.0),
      (12.0, 18.0),
      (28.0, 18.0),
      (28.0, 12.0),
    ]
    .map(|(x, y)| Point::new(x, y))
  );
  assert_eq!(outline(4.0, LineJoin::Bevel).points().len(), 12);
  assert_eq!(
    outline(4.0, LineJoin::Round).points()[0],
    Point::new(8.0, 20.0)
  );
  assert_eq!(outline(12.0, LineJoin::Miter).points().len(), 4);
}

#[test]
fn a_heavy_conic_whose_halves_nearly_meet_splits_into_two_lines() {
  let conic = Conic {
    points: [
      Point::new(0.0, 0.0),
      Point::new(1000.0, 0.0),
      Point::new(1000.0, 1000.0),
    ],
    weight: 1e7,
  };

  assert_eq!(conic.to_quads(CONIC_TOLERANCE).len(), 4);
}

#[test]
fn a_transformed_rect_maps_its_own_edges() {
  let rect = Rect {
    left: -1e8,
    top: 0.0,
    right: 1.0,
    bottom: 1.0,
  };

  assert_eq!(rect.transformed(Affine::IDENTITY), Some(rect));
}

#[test]
fn a_path_from_an_inverted_rect_sorts_its_bounds() {
  let path = Path::from_rect(Rect {
    left: 10.0,
    top: 20.0,
    right: 0.0,
    bottom: 5.0,
  });

  assert_eq!(
    path.bounds(),
    Rect {
      left: 0.0,
      top: 5.0,
      right: 10.0,
      bottom: 20.0,
    }
  );
  assert_eq!(path.points()[0], Point::new(10.0, 20.0));
}
