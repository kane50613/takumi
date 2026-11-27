use super::{Path, PathBuilder, Stroke, StrokeDash};
use crate::geometry::{Point, Rect};
use crate::style::{LineCap, LineJoin};

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
