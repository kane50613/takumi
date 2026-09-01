//! CSS gradient / background-image → SVG paint emission.
//!
//! Linear and radial gradients map onto native `<linearGradient>` /
//! `<radialGradient>` (stops resolved with the same logic the raster backend
//! uses, so geometry and stop placement match). Conic gradients have no SVG 1.1
//! construct, so they are approximated by a fan of solid-color wedge `<path>`s
//! sampled from the gradient's color LUT and clipped to the tile.
//!
//! `background-size`/`background-position`/`background-repeat` are resolved with
//! the same logic the raster backend uses ([`BackgroundSize::resolve`] and the
//! tile-position collectors), producing a tile rectangle and a list of tile
//! origins. A single tile is painted directly; tiling repeats are emitted as an
//! SVG `<pattern>` filling the box.

use std::{f32::consts::TAU, io};

use takumi_core::{
  context::RenderContext,
  geometry::{Point, Size},
  layout::background::{LayerTileStyle, ResolveBackgroundLayerInput, resolve_background_layer},
  paint::{
    ConicGradientTile, LinearGradientTile, RadialGradientTile, build_color_lut_with_interpolation,
    resolve_stops_along_axis,
  },
  style::{
    BackgroundImage, BackgroundRepeat, BackgroundSize, BlendMode, ColorInterpolationMethod,
    ConicGradient, LinearGradient, PositionValue, RadialGradient, ResolvedGradientStop,
  },
};

use crate::{
  APPROX_CHARS_PER_NUMBER, Frame, GradientStop, IDENTITY, Rgba, SvgDocument,
  box_model::{PathData, rect_path_data},
  image::{PRESERVE_ASPECT_NONE, data_url_for_url},
};

const CONIC_WEDGES: usize = 180;

/// A resolved background/mask layer: the per-tile rect and the tile origins on
/// each axis (the cartesian product gives every tile placement).
struct LayerPlacement {
  tile_w: f32,
  tile_h: f32,
  xs: Vec<f32>,
  ys: Vec<f32>,
}

/// Emits background/mask image layers for one node into an SVG document. Bundles
/// the render context and output doc so the layer/tile chain threads only the
/// per-tile [`Frame`] geometry, not the whole `(context, x, y, w, h, doc)` tuple.
pub(crate) struct LayerEmitter<'a, 'd> {
  context: &'a RenderContext,
  doc: &'d mut SvgDocument,
}

impl<'a, 'd> LayerEmitter<'a, 'd> {
  pub(crate) fn new(context: &'a RenderContext, doc: &'d mut SvgDocument) -> Self {
    Self { context, doc }
  }

  /// Emits every background gradient/image for a node, painted bottom-up (CSS
  /// lists the topmost layer first, so the slice is walked in reverse).
  /// `background-size`, `-position`, and `-repeat` are honored per layer (cycling
  /// the last value when shorter than the image list, matching the raster backend).
  pub(crate) fn background_images(
    &mut self,
    images: &[BackgroundImage],
    area: Frame,
    paint: Frame,
  ) -> io::Result<()> {
    self.image_layers(
      images,
      &self.context.style.background_size,
      &self.context.style.background_position,
      &self.context.style.background_repeat,
      area,
      paint,
    )
  }

  /// Emits a list of background/mask image layers honoring per-layer size/
  /// position/repeat. Shared by `background-image` and `mask-image`.
  ///
  /// `area` is the `background-origin` positioning area; `paint` is the painting
  /// (border) box that `repeat` tiles fill and overflow is clipped to, so origin
  /// only shifts placement, not the clip — matching the raster backend.
  pub(crate) fn image_layers(
    &mut self,
    images: &[BackgroundImage],
    sizes: &[BackgroundSize],
    positions: &[PositionValue],
    repeats: &[BackgroundRepeat],
    area: Frame,
    paint: Frame,
  ) -> io::Result<()> {
    if paint.w <= 0.0 || paint.h <= 0.0 {
      return Ok(());
    }
    let last_size = sizes.last().copied().unwrap_or_default();
    let last_position = positions.last().copied().unwrap_or_default();
    let last_repeat = repeats.last().copied().unwrap_or_default();

    for (index, image) in images.iter().enumerate().rev() {
      if !image.paints() {
        continue;
      }
      let size = sizes.get(index).copied().unwrap_or(last_size);
      let position = positions.get(index).copied().unwrap_or(last_position);
      let repeat = repeats.get(index).copied().unwrap_or(last_repeat);
      self.layer(image, size, position, repeat, area, paint)?;
    }
    Ok(())
  }

  fn layer(
    &mut self,
    image: &BackgroundImage,
    size: BackgroundSize,
    position: PositionValue,
    repeat: BackgroundRepeat,
    area: Frame,
    paint: Frame,
  ) -> io::Result<()> {
    let Some(placement) =
      resolve_placement(image, size, position, repeat, self.context, area, paint)
    else {
      return Ok(());
    };
    if placement.tile_w <= 0.0 || placement.tile_h <= 0.0 {
      return Ok(());
    }

    // Tile positions are relative to the painting box, so origin only shifts
    // placement while the clip stays the painting box.
    if placement.xs.len() == 1 && placement.ys.len() == 1 {
      let tile = Frame::new(
        paint.x + placement.xs[0],
        paint.y + placement.ys[0],
        placement.tile_w,
        placement.tile_h,
      );
      // A `cover`/positioned/origin-shifted tile can extend past the painting box;
      // clip it so it does not bleed outside the element (matching the raster backend).
      let overflows = tile.x < paint.x - 1e-3
        || tile.y < paint.y - 1e-3
        || tile.x + tile.w > paint.x + paint.w + 1e-3
        || tile.y + tile.h > paint.y + paint.h + 1e-3;
      if overflows {
        let token = self.begin_box_clip(paint)?;
        self.tile(image, tile)?;
        return self.doc.end_group(token);
      }
      return self.tile(image, tile);
    }

    self.tiled_pattern(image, paint, &placement)
  }

  /// Opens a group clipped to the layer box so tiles that extend past the edges
  /// (cover/positioned/repeated) don't bleed outside it.
  fn begin_box_clip(&mut self, frame: Frame) -> io::Result<crate::GroupToken> {
    let clip = self
      .doc
      .clip_path(&rect_path_data(frame.x, frame.y, frame.w, frame.h))?;
    self.doc.begin_group(IDENTITY, 1.0, Some(&clip), None)
  }

  fn tiled_pattern(
    &mut self,
    image: &BackgroundImage,
    paint_box: Frame,
    placement: &LayerPlacement,
  ) -> io::Result<()> {
    // A `<pattern>` repeats on both axes, so only use it when both axes have
    // multiple evenly-spaced tiles; otherwise a single row/column would wrongly
    // repeat on the other axis, so emit the explicit grid.
    let even_x = is_even_step(&placement.xs, placement.tile_w);
    let even_y = is_even_step(&placement.ys, placement.tile_h);
    if let (Some(step_x), Some(step_y)) = (even_x, even_y)
      && placement.xs.len() > 1
      && placement.ys.len() > 1
    {
      let origin_x = paint_box.x + placement.xs[0];
      let origin_y = paint_box.y + placement.ys[0];
      let (token, paint) = self.doc.begin_pattern(origin_x, origin_y, step_x, step_y)?;
      self.tile(
        image,
        Frame::new(0.0, 0.0, placement.tile_w, placement.tile_h),
      )?;
      self.doc.end_pattern(token)?;
      return self
        .doc
        .rect_paint(paint_box.x, paint_box.y, paint_box.w, paint_box.h, &paint);
    }

    // Explicit tile grid, clipped to the box so edge tiles don't bleed outside.
    let token = self.begin_box_clip(paint_box)?;
    for &ty in &placement.ys {
      for &tx in &placement.xs {
        self.tile(
          image,
          Frame::new(
            paint_box.x + tx,
            paint_box.y + ty,
            placement.tile_w,
            placement.tile_h,
          ),
        )?;
      }
    }
    self.doc.end_group(token)
  }

  /// Paints one tile of a layer into `rect`.
  fn tile(&mut self, image: &BackgroundImage, rect: Frame) -> io::Result<()> {
    match image {
      BackgroundImage::Linear(gradient) => self.linear(gradient, rect),
      BackgroundImage::Radial(gradient) => self.radial(gradient, rect),
      BackgroundImage::Conic(gradient) => self.conic(gradient, rect),
      BackgroundImage::Url(url) => self.url(url, rect),
      BackgroundImage::None => Ok(()),
    }
  }

  fn url(&mut self, url: &str, rect: Frame) -> io::Result<()> {
    let Some(href) = data_url_for_url(url, self.context) else {
      return Ok(());
    };
    self.doc.image(
      rect.x,
      rect.y,
      rect.w,
      rect.h,
      &href,
      Some(PRESERVE_ASPECT_NONE),
    )
  }

  fn linear(&mut self, gradient: &LinearGradient, rect: Frame) -> io::Result<()> {
    let Frame { x, y, w, h } = rect;
    let tile = LinearGradientTile::new(
      gradient,
      w as u32,
      h as u32,
      &self.context.sizing,
      self.context.current_color,
      false,
    );
    let resolved = resolve_stops_along_axis(
      &gradient.stops,
      tile.axis_length.max(1e-6),
      &self.context.sizing,
      self.context.current_color,
    );
    if resolved.is_empty() {
      return Ok(());
    }

    // Map an axis position (px from one edge of the gradient line) to a point.
    let max_extent = tile.axis_length / 2.0;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let point_at = |t: f32| {
      (
        cx + (t - max_extent) * tile.dir_x,
        cy + (t - max_extent) * tile.dir_y,
      )
    };

    let (t0, t1, base, span) = if gradient.repeating {
      let first = resolved.first().map_or(0.0, |s| s.position);
      let last = resolved.last().map_or(tile.axis_length, |s| s.position);
      (first, last, first, last - first)
    } else {
      (0.0, tile.axis_length, 0.0, tile.axis_length)
    };

    let stops = if gradient.repeating {
      svg_stops(&resolved, base, span)
    } else {
      lut_svg_stops(&resolved, tile.axis_length, gradient.interpolation)
    };
    let paint = self
      .doc
      .linear_gradient(point_at(t0), point_at(t1), gradient.repeating, &stops)?;
    self.doc.rect_paint(x, y, w, h, &paint)
  }

  fn radial(&mut self, gradient: &RadialGradient, rect: Frame) -> io::Result<()> {
    let Frame { x, y, w, h } = rect;
    let tile = RadialGradientTile::new(
      gradient,
      w as u32,
      h as u32,
      &self.context.sizing,
      self.context.current_color,
      false,
    );
    let resolved = resolve_stops_along_axis(
      &gradient.stops,
      tile.radius_scale.max(1e-6),
      &self.context.sizing,
      self.context.current_color,
    );
    if resolved.is_empty() {
      return Ok(());
    }

    let radius_x = tile.inv_radius_x.recip();
    let radius_y = tile.inv_radius_y.recip();
    let (r, base, span) = if gradient.repeating {
      let first = resolved.first().map_or(0.0, |s| s.position);
      let last = resolved.last().map_or(tile.radius_scale, |s| s.position);
      ((last - first).max(1e-6), first, last - first)
    } else {
      (tile.radius_scale, 0.0, tile.radius_scale)
    };
    let scale = (
      (radius_x / tile.radius_scale.max(1e-6)).max(1e-6),
      (radius_y / tile.radius_scale.max(1e-6)).max(1e-6),
    );

    let stops = if gradient.repeating {
      svg_stops(&resolved, base, span)
    } else {
      lut_svg_stops(&resolved, tile.radius_scale, gradient.interpolation)
    };
    let paint = self.doc.radial_gradient(
      (x + tile.cx, y + tile.cy),
      r,
      scale,
      gradient.repeating,
      &stops,
    )?;
    self.doc.rect_paint(x, y, w, h, &paint)
  }

  fn conic(&mut self, gradient: &ConicGradient, rect: Frame) -> io::Result<()> {
    let Frame { x, y, w, h } = rect;
    let tile = ConicGradientTile::new(
      gradient,
      w as u32,
      h as u32,
      &self.context.sizing,
      self.context.current_color,
      false,
    );
    let lut_len = tile.color_lut.len();
    if lut_len == 0 {
      return Ok(());
    }

    let (ccx, ccy) = (x + tile.cx, y + tile.cy);
    let radius = [(x, y), (x + w, y), (x, y + h), (x + w, y + h)]
      .into_iter()
      .map(|(px, py)| (px - ccx).hypot(py - ccy))
      .fold(0.0_f32, f32::max);

    let clip = self.doc.clip_path(&rect_path_data(x, y, w, h))?;
    let group = self.doc.begin_group(IDENTITY, 1.0, Some(&clip), None)?;
    for i in 0..CONIC_WEDGES {
      let a0 = i as f32 / CONIC_WEDGES as f32 * TAU;
      let a1 = (i + 1) as f32 / CONIC_WEDGES as f32 * TAU;
      let mid = (a0 + a1) / 2.0;
      let adjusted = (mid - tile.start_rad).rem_euclid(TAU);
      let idx = tile.lut_index_for_adjusted_angle_with_len(adjusted, lut_len);
      let color = tile.color_lut[idx].demultiply();
      let fill = Rgba([color.red(), color.green(), color.blue(), color.alpha()]);
      if fill.0[3] == 0 {
        continue;
      }
      let (x0, y0) = (ccx + radius * a0.sin(), ccy - radius * a0.cos());
      let (x1, y1) = (ccx + radius * a1.sin(), ccy - radius * a1.cos());
      let mut wedge = PathData::with_capacity(6 * APPROX_CHARS_PER_NUMBER);
      wedge.command(b'M');
      wedge.pair(ccx, ccy);
      wedge.command(b'L');
      wedge.pair(x0, y0);
      wedge.pair(x1, y1);
      wedge.close();
      self.doc.path(&wedge.into_string(), fill, false)?;
    }
    self.doc.end_group(group)
  }
}

/// Returns the uniform step between positions (tile size when there is a single
/// tile) if the positions are equally spaced, else `None`.
fn is_even_step(positions: &[f32], tile_size: f32) -> Option<f32> {
  match positions {
    [] => None,
    [_] => Some(tile_size),
    [first, second, ..] => {
      let step = second - first;
      positions
        .windows(2)
        .all(|w| (w[1] - w[0] - step).abs() < 0.5)
        .then_some(step)
    }
  }
}

/// Resolves a layer's tile size and per-axis tile origins (box-relative).
fn resolve_placement(
  image: &BackgroundImage,
  size: BackgroundSize,
  position: PositionValue,
  repeat: BackgroundRepeat,
  context: &RenderContext,
  area: Frame,
  paint: Frame,
) -> Option<LayerPlacement> {
  let geometry = resolve_background_layer(
    image,
    LayerTileStyle {
      pos: position,
      size,
      repeat,
      blend_mode: BlendMode::Normal,
    },
    ResolveBackgroundLayerInput {
      area: Size {
        width: area.w.round().max(0.0) as u32,
        height: area.h.round().max(0.0) as u32,
      },
      paint: Size {
        width: paint.w.round().max(0.0) as u32,
        height: paint.h.round().max(0.0) as u32,
      },
      origin_offset: Point {
        x: (area.x - paint.x).round() as i32,
        y: (area.y - paint.y).round() as i32,
      },
      context,
    },
  )?;

  Some(LayerPlacement {
    tile_w: geometry.tile_width as f32,
    tile_h: geometry.tile_height as f32,
    xs: geometry.xs.iter().map(|x| *x as f32).collect(),
    ys: geometry.ys.iter().map(|y| *y as f32).collect(),
  })
}

fn svg_stops(stops: &[ResolvedGradientStop], base: f32, span: f32) -> Vec<GradientStop> {
  let span = span.max(1e-6);
  stops
    .iter()
    .map(|stop| GradientStop {
      offset: ((stop.position - base) / span).clamp(0.0, 1.0),
      color: Rgba(stop.color.0),
    })
    .collect()
}

/// Number of stops sampled from the gradient color LUT for vector emission.
const GRADIENT_LUT_STOPS: usize = 64;

/// Builds dense SVG gradient stops by sampling takumi's interpolated color LUT,
/// baking the gradient's interpolation color space (e.g. OKLCH) into evenly-spaced
/// sRGB stops. SVG only interpolates between stops in sRGB, so sampling the LUT is
/// how the vector output matches the raster backend for non-sRGB interpolation.
///
/// CSS hard stops (two stops at the same position) are a discontinuity the uniform
/// LUT would smear into a ~1-cell ramp, so a coincident stop pair carrying the
/// exact before/after colors is injected at each hard-stop offset and the LUT
/// samples bridging it are dropped to keep the transition sharp.
fn lut_svg_stops(
  resolved: &[ResolvedGradientStop],
  axis_length: f32,
  interpolation: ColorInterpolationMethod,
) -> Vec<GradientStop> {
  let lut = build_color_lut_with_interpolation(
    resolved,
    axis_length.max(1e-6),
    GRADIENT_LUT_STOPS,
    interpolation,
  );
  if lut.len() <= 1 {
    return svg_stops(resolved, 0.0, axis_length);
  }
  let span = axis_length.max(1e-6);
  let cell = 1.0 / (lut.len() - 1) as f32;
  let demul = |premultiplied: &tiny_skia::PremultipliedColorU8| {
    let color = premultiplied.demultiply();
    Rgba([color.red(), color.green(), color.blue(), color.alpha()])
  };

  // Hard stops: adjacent resolved stops with (near-)equal positions.
  let mut hard_stops = Vec::new();
  for pair in resolved.windows(2) {
    let (a, b) = (&pair[0], &pair[1]);
    if (b.position - a.position).abs() <= 1e-3 {
      hard_stops.push((
        (a.position / span).clamp(0.0, 1.0),
        Rgba(a.color.0),
        Rgba(b.color.0),
      ));
    }
  }

  let mut stops: Vec<GradientStop> = lut
    .iter()
    .enumerate()
    .filter(|(index, _)| {
      let offset = *index as f32 / (lut.len() - 1) as f32;
      // Drop LUT samples that straddle a hard stop; the injected pair covers it.
      !hard_stops
        .iter()
        .any(|(boundary, ..)| (offset - boundary).abs() < cell)
    })
    .map(|(index, premultiplied)| GradientStop {
      offset: index as f32 / (lut.len() - 1) as f32,
      color: demul(premultiplied),
    })
    .collect();

  for (boundary, before, after) in hard_stops {
    stops.push(GradientStop {
      offset: boundary,
      color: before,
    });
    stops.push(GradientStop {
      offset: boundary,
      color: after,
    });
  }
  stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
  stops
}
