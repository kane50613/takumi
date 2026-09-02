use takumi_core::{
  geometry::{Point, Rect, Size},
  layout::border::{BorderProperties, BorderSide, PaintedSide},
};

use crate::{
  Canvas, Cap, DashPattern, Fill, MaskCompositeColor, MaskSamplingOptions, PaintSource,
  PathBuilder, Placement, Stroke, Style, intersect_alpha_masks, render_mask,
  style::{Affine, BlendMode, BorderStyle, Color, ImageScalingAlgorithm},
};

/// Canvas-backed rasterization of [`BorderProperties`].
pub(crate) fn paint_border(
  properties: BorderProperties,
  canvas: &mut Canvas,
  border_box: Size<f32>,
  transform: Affine,
  clip_image: Option<PaintSource<'_>>,
) {
  if let Some(clip_image) = &clip_image {
    assert_eq!(
      (clip_image.width(), clip_image.height()),
      (border_box.width as u32, border_box.height as u32),
    );
  }

  if !properties.has_visible_sides() {
    return;
  }

  if draw_uniform_fast_path(properties, canvas, border_box, transform, clip_image) {
    return;
  }

  let inverse = if clip_image.is_some() {
    transform.invert()
  } else {
    None
  };
  let mut paint = SidePaintContext {
    canvas,
    transform,
    clip_image,
    inverse,
  };

  let mut border = properties;
  border.width = properties.visible_side_widths();

  for side in border.painted_sides() {
    draw_visible_side(border, &mut paint, side, border_box);
  }
}

fn draw_uniform_fast_path(
  border: BorderProperties,
  canvas: &mut Canvas,
  border_box: Size<f32>,
  transform: Affine,
  clip_image: Option<PaintSource<'_>>,
) -> bool {
  let Some(color) = border.has_uniform_visible_color() else {
    return false;
  };

  if border.visible_sides_match(BorderStyle::Solid) {
    let mut solid = border;
    solid.width = border.visible_side_widths();
    let mut paths = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);
    solid.append_border_ring_commands(&mut paths, border_box);
    let (mask, placement) = render_mask(
      &paths,
      Some(transform),
      Some(Fill::EvenOdd.into()),
      Some(canvas.viewport()),
    );

    paint_mask(
      canvas,
      &mask,
      placement,
      color,
      clip_image,
      transform,
      border.image_rendering,
    );
    return true;
  }

  if border.visible_sides_match(BorderStyle::Double) {
    let mut double = border;
    double.width = border.visible_side_widths();
    draw_uniform_double(double, canvas, border_box, transform, clip_image, color);
    return true;
  }

  if border.is_uniform_all_sides_style(BorderStyle::Dashed) {
    draw_uniform_pattern(
      border,
      canvas,
      border_box,
      transform,
      clip_image,
      color,
      BorderStyle::Dashed,
    );
    return true;
  }

  if border.is_uniform_all_sides_style(BorderStyle::Dotted) {
    draw_uniform_pattern(
      border,
      canvas,
      border_box,
      transform,
      clip_image,
      color,
      BorderStyle::Dotted,
    );
    return true;
  }

  false
}

fn draw_visible_side(
  border: BorderProperties,
  paint: &mut SidePaintContext<'_, '_>,
  side: PaintedSide,
  border_box: Size<f32>,
) {
  if matches!(side.style, BorderStyle::Dashed | BorderStyle::Dotted) {
    draw_side_pattern_border(border, paint, side.side, border_box, side.color, side.style);
    return;
  }
  for band in border.side_bands(side) {
    draw_side_band(
      border, paint, side.side, border_box, band.inset, band.width, band.color,
    );
  }
}

fn draw_uniform_double(
  border: BorderProperties,
  canvas: &mut Canvas,
  border_box: Size<f32>,
  transform: Affine,
  clip_image: Option<PaintSource<'_>>,
  color: Color,
) {
  let stripe_width = border.width.map(|value| value / 3.0);
  let mut outer = border;
  outer.width = stripe_width;

  let mut paths = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 4);
  outer.append_border_ring_commands(&mut paths, border_box);

  let inset = border.width.map(|value| value * (2.0 / 3.0));
  let mut inner = border;
  inner.width = stripe_width;
  inner.expand_by(inset.map(|value| -value));
  inner.append_border_ring_commands_at(&mut paths, border_box.inset(inset), inset.top_left());

  let (mask, placement) = render_mask(
    &paths,
    Some(transform),
    Some(Fill::EvenOdd.into()),
    Some(canvas.viewport()),
  );
  paint_mask(
    canvas,
    &mask,
    placement,
    color,
    clip_image,
    transform,
    border.image_rendering,
  );
}

fn draw_uniform_pattern(
  border: BorderProperties,
  canvas: &mut Canvas,
  border_box: Size<f32>,
  transform: Affine,
  clip_image: Option<PaintSource<'_>>,
  color: Color,
  style: BorderStyle,
) {
  let width = border.width.top;
  if width <= 0.0 {
    return;
  }

  let half_width = border.width.map(|v| v / 2.0);
  let mut center_rect = border;
  center_rect.expand_by(half_width.map(|v| -v));

  let center_size = border_box.inset(half_width);
  let center_offset = half_width.top_left();

  let mut paths = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);
  center_rect.append_mask_commands(&mut paths, center_size, center_offset);

  let perimeter = center_rect.approximate_rounded_rect_perimeter(center_size);

  let stroke = compute_side_stroke(width, style, perimeter, true);

  let (mask, placement) = render_mask(
    &paths,
    Some(transform),
    Some(Style::Stroke(stroke)),
    Some(canvas.viewport()),
  );

  paint_mask(
    canvas,
    &mask,
    placement,
    color,
    clip_image,
    transform,
    border.image_rendering,
  );
}

fn draw_side_band(
  border: BorderProperties,
  paint: &mut SidePaintContext<'_, '_>,
  side: BorderSide,
  border_box: Size<f32>,
  inset: Rect<f32>,
  width: Rect<f32>,
  color: Color,
) {
  if border_box.width <= 0.0 || border_box.height <= 0.0 {
    return;
  }

  let mut band = border;
  band.width = width;

  let band_box = border_box.inset(inset);
  if band_box.width <= 0.0 || band_box.height <= 0.0 {
    return;
  }
  let offset = inset.top_left();
  band.expand_by(inset.map(|value| -value));

  if band.is_zero() {
    let mut paths = Vec::with_capacity(5);
    band.append_side_polygon_commands_at(side, &mut paths, band_box, offset);
    let (mask, placement) = render_mask(
      &paths,
      Some(paint.transform),
      Some(Fill::NonZero.into()),
      Some(paint.canvas.viewport()),
    );
    paint_mask_with_inverse(
      paint.canvas,
      &mask,
      placement,
      color,
      paint.clip_image,
      paint.inverse,
      border.image_rendering,
    );
    return;
  }

  let mut ring_paths = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);
  band.append_border_ring_commands_at(&mut ring_paths, band_box, offset);
  let (ring_mask, ring_placement) = render_mask(
    &ring_paths,
    Some(paint.transform),
    Some(Fill::EvenOdd.into()),
    Some(paint.canvas.viewport()),
  );

  if !ring_mask.is_empty() {
    let mut clip_paths = Vec::with_capacity(5);
    band.append_side_clip_polygon_commands_at(side, &mut clip_paths, band_box, offset);
    let (clip_mask, clip_placement) = render_mask(
      &clip_paths,
      Some(paint.transform),
      Some(Fill::NonZero.into()),
      Some(paint.canvas.viewport()),
    );

    if let Some((mask, placement)) =
      intersect_alpha_masks(&ring_mask, ring_placement, &clip_mask, clip_placement)
    {
      paint_mask_with_inverse(
        paint.canvas,
        &mask,
        placement,
        color,
        paint.clip_image,
        paint.inverse,
        border.image_rendering,
      );
    }
  }
}

fn draw_side_pattern_border(
  border: BorderProperties,
  paint: &mut SidePaintContext<'_, '_>,
  side: BorderSide,
  border_box: Size<f32>,
  color: Color,
  style: BorderStyle,
) {
  let line = SidePatternLine::from_border(border.width, border_box, side);
  if line.width <= 0.0 || line.end <= line.start {
    return;
  }

  let mut path = Vec::with_capacity(2);
  if line.is_horizontal {
    path.move_to((line.start, line.fixed));
    path.line_to((line.end, line.fixed));
  } else {
    path.move_to((line.fixed, line.start));
    path.line_to((line.fixed, line.end));
  }

  let stroke = compute_side_stroke(line.width, style, line.end - line.start, false);
  let (pattern_mask, pattern_placement) = render_mask(
    &path,
    Some(paint.transform),
    Some(Style::Stroke(stroke)),
    Some(paint.canvas.viewport()),
  );

  if !pattern_mask.is_empty() {
    let mut ring_path = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);
    border.append_border_ring_commands(&mut ring_path, border_box);
    let (ring_mask, ring_placement) = render_mask(
      &ring_path,
      Some(paint.transform),
      Some(Fill::EvenOdd.into()),
      Some(paint.canvas.viewport()),
    );

    let mut clip_path = Vec::with_capacity(5);
    border.append_side_clip_polygon_commands_at(side, &mut clip_path, border_box, Point::ZERO);
    let (clip_mask, clip_placement) = render_mask(
      &clip_path,
      Some(paint.transform),
      Some(Fill::NonZero.into()),
      Some(paint.canvas.viewport()),
    );

    if !ring_mask.is_empty()
      && let Some((mask, placement)) =
        intersect_alpha_masks(&pattern_mask, pattern_placement, &clip_mask, clip_placement)
      && let Some((mask, placement)) =
        intersect_alpha_masks(&mask, placement, &ring_mask, ring_placement)
    {
      paint_mask_with_inverse(
        paint.canvas,
        &mask,
        placement,
        color,
        paint.clip_image,
        paint.inverse,
        border.image_rendering,
      );
    }
  }
}

fn compute_side_stroke(width: f32, style: BorderStyle, length: f32, closed: bool) -> Stroke {
  let mut stroke = Stroke::new(width);
  if let Some(dash) = style.dash_pattern(width, length, closed) {
    if dash.round_cap {
      stroke.cap = Cap::Round;
    }
    stroke.dash = Some(DashPattern {
      intervals: dash.intervals,
      offset: 0.0,
    });
  }
  stroke
}

#[derive(Clone, Copy)]
struct SidePatternLine {
  width: f32,
  is_horizontal: bool,
  fixed: f32,
  start: f32,
  end: f32,
}

impl SidePatternLine {
  fn from_border(width: Rect<f32>, border_box: Size<f32>, side: BorderSide) -> Self {
    match side {
      BorderSide::Top => Self {
        width: width.top,
        is_horizontal: true,
        fixed: width.top / 2.0,
        start: width.left / 2.0,
        end: border_box.width - width.right / 2.0,
      },
      BorderSide::Right => Self {
        width: width.right,
        is_horizontal: false,
        fixed: border_box.width - width.right / 2.0,
        start: width.top / 2.0,
        end: border_box.height - width.bottom / 2.0,
      },
      BorderSide::Bottom => Self {
        width: width.bottom,
        is_horizontal: true,
        fixed: border_box.height - width.bottom / 2.0,
        start: width.left / 2.0,
        end: border_box.width - width.right / 2.0,
      },
      BorderSide::Left => Self {
        width: width.left,
        is_horizontal: false,
        fixed: width.left / 2.0,
        start: width.top / 2.0,
        end: border_box.height - width.bottom / 2.0,
      },
    }
  }
}

struct SidePaintContext<'canvas, 'source> {
  canvas: &'canvas mut Canvas,
  transform: Affine,
  clip_image: Option<PaintSource<'source>>,
  inverse: Option<Affine>,
}

fn paint_mask(
  canvas: &mut Canvas,
  mask: &[u8],
  placement: Placement,
  color: Color,
  clip_image: Option<PaintSource<'_>>,
  transform: Affine,
  image_rendering: ImageScalingAlgorithm,
) {
  paint_mask_with_inverse(
    canvas,
    mask,
    placement,
    color,
    clip_image,
    transform.invert(),
    image_rendering,
  );
}

fn paint_mask_with_inverse(
  canvas: &mut Canvas,
  mask: &[u8],
  placement: Placement,
  color: Color,
  clip_image: Option<PaintSource<'_>>,
  inverse: Option<Affine>,
  image_rendering: ImageScalingAlgorithm,
) {
  if let Some(clip_image) = clip_image {
    let Some(inverse) = inverse else {
      return;
    };
    canvas.composite_mask_source(
      mask,
      placement,
      clip_image,
      MaskCompositeColor::source_over_color(color),
      MaskSamplingOptions {
        canvas_to_source: inverse,
        sample_bias: Point::ZERO,
        algorithm: image_rendering,
      },
      BlendMode::Normal,
    );
  } else {
    canvas.draw_mask(mask, placement, color, BlendMode::Normal);
  }
}

#[cfg(test)]
mod tests {
  use takumi_core::{
    geometry::{Rect, Size},
    layout::border::BorderProperties,
    style::{Affine, BorderStyle, Color, ImageScalingAlgorithm, Sides, SpacePair},
  };

  use super::{Cap, compute_side_stroke, paint_border};
  use crate::Canvas;

  fn test_border(style: BorderStyle, width: f32) -> BorderProperties {
    BorderProperties {
      width: Rect {
        top: width,
        right: width,
        bottom: width,
        left: width,
      },
      color: Rect {
        top: Color([255, 0, 0, 255]),
        right: Color([255, 0, 0, 255]),
        bottom: Color([255, 0, 0, 255]),
        left: Color([255, 0, 0, 255]),
      },
      radius: Sides([SpacePair::from_single(0.0); 4]),
      style: Rect {
        top: style,
        right: style,
        bottom: style,
        left: style,
      },
      image_rendering: ImageScalingAlgorithm::Auto,
      collapsed: false,
      shape: Sides::default(),
    }
  }

  #[test]
  fn solid_border_draws_continuous_edge() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });

    paint_border(
      test_border(BorderStyle::Solid, 4.0),
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));
    assert!((8..40).all(|x| image.get_pixel(x, 2).0[3] > 0));
  }

  #[test]
  fn hidden_border_does_not_draw() {
    let mut canvas = Canvas::new(Size {
      width: 24,
      height: 24,
    });

    paint_border(
      test_border(BorderStyle::Hidden, 4.0),
      &mut canvas,
      Size {
        width: 24.0,
        height: 24.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));
    assert!(image.pixels().all(|pixel| pixel.0[3] == 0));
  }

  #[test]
  fn dashed_border_draws_pattern() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });

    paint_border(
      test_border(BorderStyle::Dashed, 4.0),
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    // Check top border line (y=2)
    // It should have some transparent pixels (gaps) and some opaque pixels (dashes)
    let row: Vec<u8> = (0..48).map(|x| image.get_pixel(x, 2).0[3]).collect();
    let has_opaque = row.iter().any(|&a| a > 0);
    let has_transparent = row.iter().skip(8).take(32).any(|&a| a == 0);

    assert!(has_opaque, "Dashed border should have opaque pixels");
    assert!(
      has_transparent,
      "Dashed border should have transparent gaps"
    );
  }

  #[test]
  fn dotted_border_draws_pattern() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });

    paint_border(
      test_border(BorderStyle::Dotted, 4.0),
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    let row: Vec<u8> = (0..48).map(|x| image.get_pixel(x, 2).0[3]).collect();
    let has_opaque = row.iter().any(|&a| a > 0);
    let has_transparent = row.iter().skip(8).take(32).any(|&a| a == 0);

    assert!(has_opaque, "Dotted border should have opaque pixels");
    assert!(
      has_transparent,
      "Dotted border should have transparent gaps"
    );
  }

  #[test]
  fn dotted_border_thin_width_uses_zero_dash_length() {
    let stroke = compute_side_stroke(2.0, BorderStyle::Dotted, 48.0, false);
    let Some(dash_pattern) = stroke.dash else {
      unreachable!("thin dotted stroke should produce a dash pattern");
    };
    assert_eq!(stroke.cap, Cap::Round);
    assert_eq!(dash_pattern.intervals[0], 0.0);
    assert!(dash_pattern.intervals[1] > 0.0);
  }

  #[test]
  fn dashed_border_top_only_draws_pattern() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });
    let mut border = test_border(BorderStyle::Dashed, 0.0);
    border.width.top = 4.0;

    paint_border(
      border,
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));
    let top_row: Vec<u8> = (8..40).map(|x| image.get_pixel(x, 2).0[3]).collect();

    assert!(
      top_row.iter().any(|&alpha| alpha > 0),
      "Top dashed side should contain opaque pixels"
    );
    assert!(
      top_row.contains(&0),
      "Top dashed side should contain transparent gaps"
    );
    assert_eq!(
      image.get_pixel(24, 45).0[3],
      0,
      "Bottom side should stay transparent for top-only dashed border"
    );
    assert_eq!(
      image.get_pixel(2, 24).0[3],
      0,
      "Left side should stay transparent for top-only dashed border"
    );
    assert_eq!(
      image.get_pixel(45, 24).0[3],
      0,
      "Right side should stay transparent for top-only dashed border"
    );
  }

  #[test]
  fn dotted_border_left_only_draws_pattern() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });
    let mut border = test_border(BorderStyle::Dotted, 0.0);
    border.width.left = 4.0;

    paint_border(
      border,
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));
    let left_column: Vec<u8> = (8..40).map(|y| image.get_pixel(2, y).0[3]).collect();

    assert!(
      left_column.iter().any(|&alpha| alpha > 0),
      "Left dotted side should contain opaque pixels"
    );
    assert!(
      left_column.contains(&0),
      "Left dotted side should contain transparent gaps"
    );
    assert_eq!(
      image.get_pixel(24, 2).0[3],
      0,
      "Top side should stay transparent for left-only dotted border"
    );
    assert_eq!(
      image.get_pixel(45, 24).0[3],
      0,
      "Right side should stay transparent for left-only dotted border"
    );
    assert_eq!(
      image.get_pixel(24, 45).0[3],
      0,
      "Bottom side should stay transparent for left-only dotted border"
    );
  }

  #[test]
  fn solid_fast_path_skips_hidden_side_with_positive_width() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });
    let mut border = test_border(BorderStyle::Solid, 4.0);
    border.style.top = BorderStyle::Hidden;

    paint_border(
      border,
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    assert_eq!(
      image.get_pixel(24, 2).0[3],
      0,
      "Hidden top side should stay transparent"
    );
    let right_band_has_ink = (44..48).any(|x| image.get_pixel(x, 24).0[3] > 0);
    assert!(
      right_band_has_ink,
      "Visible right side should still be painted"
    );
  }

  #[test]
  fn double_fast_path_skips_hidden_side_with_positive_width() {
    let mut canvas = Canvas::new(Size {
      width: 48,
      height: 48,
    });
    let mut border = test_border(BorderStyle::Double, 6.0);
    border.style.top = BorderStyle::Hidden;

    paint_border(
      border,
      &mut canvas,
      Size {
        width: 48.0,
        height: 48.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    assert_eq!(
      image.get_pixel(24, 2).0[3],
      0,
      "Hidden top side should stay transparent"
    );
    let right_band_has_ink = (42..48).any(|x| image.get_pixel(x, 24).0[3] > 0);
    assert!(
      right_band_has_ink,
      "Visible right side should still be painted"
    );
  }

  #[test]
  fn solid_fallback_ignores_hidden_neighbor_widths() {
    let mut canvas = Canvas::new(Size {
      width: 64,
      height: 64,
    });
    let mut border = test_border(BorderStyle::Hidden, 0.0);
    border.style.top = BorderStyle::Solid;
    border.width.top = 8.0;
    border.style.right = BorderStyle::Dashed;
    border.width.right = 8.0;
    border.width.left = 24.0;

    paint_border(
      border,
      &mut canvas,
      Size {
        width: 64.0,
        height: 64.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    assert!(
      image.get_pixel(4, 3).0[3] > 0,
      "Visible top side should not be clipped by hidden left width"
    );
    assert_eq!(
      image.get_pixel(3, 32).0[3],
      0,
      "Hidden left side should stay transparent"
    );
  }

  #[test]
  fn oversized_solid_border_fills_without_panicking() {
    let mut canvas = Canvas::new(Size {
      width: 20,
      height: 20,
    });
    let border = test_border(BorderStyle::Solid, 40.0);

    paint_border(
      border,
      &mut canvas,
      Size {
        width: 20.0,
        height: 20.0,
      },
      Affine::IDENTITY,
      None,
    );

    let image = canvas
      .into_inner()
      .unwrap_or_else(|error| unreachable!("test canvas should be readable: {error}"));

    assert!(
      image.get_pixel(10, 10).0[3] > 0,
      "Oversized border should still render a valid filled mask"
    );
  }
}
