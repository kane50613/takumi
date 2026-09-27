//! CSS gradient / background-image → SVG paint emission.

use std::{f32::consts::TAU, io};

use takumi_core::{
  context::RenderContext,
  geometry::Rect,
  layout::background_image_geometry::{BackgroundImageGeometry, BackgroundLayer},
  paint::{ConicGradientTile, SrgbStop},
  style::{
    BackgroundImage, BlendMode, ConicGradient, FillRule, LinearGradient, RadialGradient, ToCss,
  },
};

use crate::{
  APPROX_CHARS_PER_NUMBER, Frame, GradientStop, Rgba, SvgDocument,
  box_model::PathData,
  image::{PRESERVE_ASPECT_NONE, data_url_for_url},
};

const CONIC_WEDGES: usize = 180;

/// Emits background/mask image layers for one node into an SVG document.
pub(crate) struct LayerEmitter<'a, 'd> {
  context: &'a RenderContext,
  doc: &'d mut SvgDocument,
}

impl<'a, 'd> LayerEmitter<'a, 'd> {
  pub(crate) fn new(context: &'a RenderContext, doc: &'d mut SvgDocument) -> Self {
    Self { context, doc }
  }

  /// Emits resolved background or mask layers, bottom first, positioned in `area` and painted
  /// over `paint`.
  pub(crate) fn layers(
    &mut self,
    layers: &[BackgroundLayer<'_>],
    area: Frame,
    paint: Frame,
  ) -> io::Result<()> {
    if paint.w <= 0.0 || paint.h <= 0.0 {
      return Ok(());
    }

    for layer in layers {
      let blend = (layer.blend_mode != BlendMode::Normal)
        .then(|| {
          self
            .doc
            .begin_blend_group(&layer.blend_mode.to_css_string())
        })
        .transpose()?;

      self.layer(layer.image, &layer.geometry, area, paint)?;
      if let Some(blend) = blend {
        self.doc.end_group(blend)?;
      }
    }
    Ok(())
  }

  fn layer(
    &mut self,
    image: &BackgroundImage,
    geometry: &BackgroundImageGeometry,
    area: Frame,
    paint: Frame,
  ) -> io::Result<()> {
    let tile = geometry.tile_size;
    if tile.width <= 0.0 || tile.height <= 0.0 {
      return Ok(());
    }
    let (xs, ys) = geometry.tile_origins(Rect {
      left: paint.x - area.x,
      top: paint.y - area.y,
      right: paint.x + paint.w - area.x,
      bottom: paint.y + paint.h - area.y,
    });

    // One tile in view draws on its own; a pattern only pays off for several.
    if let ([tile_x], [tile_y]) = (xs.as_slice(), ys.as_slice()) {
      let rect = Frame::new(area.x + tile_x, area.y + tile_y, tile.width, tile.height);
      // A `cover`/positioned/origin-shifted tile can extend past the painting box;
      // clip it so it does not bleed outside the element (matching the raster backend).
      let overflows = rect.x < paint.x - 1e-3
        || rect.y < paint.y - 1e-3
        || rect.x + rect.w > paint.x + paint.w + 1e-3
        || rect.y + rect.h > paint.y + paint.h + 1e-3;
      if overflows {
        let token = self.doc.begin_clipped_group(&paint.path_data())?;
        self.tile(image, rect)?;
        return self.doc.end_group(token);
      }
      return self.tile(image, rect);
    }

    let first = geometry.first_tile();
    let period = geometry.pattern_period(Rect {
      left: paint.x - area.x,
      top: paint.y - area.y,
      right: paint.x + paint.w - area.x,
      bottom: paint.y + paint.h - area.y,
    });
    let (token, pattern) = self.doc.begin_pattern(Frame::new(
      area.x + first.x,
      area.y + first.y,
      period.width,
      period.height,
    ))?;
    self.tile(image, Frame::new(0.0, 0.0, tile.width, tile.height))?;
    self.doc.end_pattern(token)?;
    self.doc.rect_paint(paint, &pattern)
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
    self.doc.image(rect, &href, Some(PRESERVE_ASPECT_NONE))
  }

  fn linear(&mut self, gradient: &LinearGradient, rect: Frame) -> io::Result<()> {
    let Frame { x, y, w, h } = rect;
    let geometry = gradient.resolve_geometry(
      w as u32,
      h as u32,
      &self.context.sizing,
      self.context.current_color,
    );
    let resolved = geometry.stops();
    if resolved.is_empty() {
      return Ok(());
    }

    let max_extent = geometry.axis_length / 2.0;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let point_at = |t: f32| {
      (
        cx + (t - max_extent) * geometry.dir_x,
        cy + (t - max_extent) * geometry.dir_y,
      )
    };

    let (t0, t1, stops) = if gradient.repeating {
      let first = resolved.first().map_or(0.0, |s| s.position);
      let last = resolved.last().map_or(geometry.axis_length, |s| s.position);
      (
        first,
        last,
        svg_stops(SrgbStop::spanned(resolved, first, last - first)),
      )
    } else {
      (
        0.0,
        geometry.axis_length,
        svg_stops(SrgbStop::sampled(
          resolved,
          geometry.axis_length,
          gradient.interpolation,
        )),
      )
    };
    let paint = self
      .doc
      .linear_gradient(point_at(t0), point_at(t1), gradient.repeating, &stops)?;
    self.doc.rect_paint(rect, &paint)
  }

  fn radial(&mut self, gradient: &RadialGradient, rect: Frame) -> io::Result<()> {
    let Frame { x, y, w, h } = rect;
    let geometry = gradient.resolve_geometry(
      w as u32,
      h as u32,
      &self.context.sizing,
      self.context.current_color,
    );
    let resolved = geometry.stops();
    if resolved.is_empty() {
      return Ok(());
    }

    let radius_x = geometry.inv_radius_x.recip();
    let radius_y = geometry.inv_radius_y.recip();
    let (r, stops) = if gradient.repeating {
      let first = resolved.first().map_or(0.0, |s| s.position);
      let last = resolved
        .last()
        .map_or(geometry.radius_scale, |s| s.position);
      (
        (last - first).max(1e-6),
        svg_stops(SrgbStop::spanned(resolved, first, last - first)),
      )
    } else {
      (
        geometry.radius_scale,
        svg_stops(SrgbStop::sampled(
          resolved,
          geometry.radius_scale,
          gradient.interpolation,
        )),
      )
    };
    let scale = (
      (radius_x / geometry.radius_scale.max(1e-6)).max(1e-6),
      (radius_y / geometry.radius_scale.max(1e-6)).max(1e-6),
    );
    let paint = self.doc.radial_gradient(
      (x + geometry.cx, y + geometry.cy),
      r,
      scale,
      gradient.repeating,
      &stops,
    )?;
    self.doc.rect_paint(rect, &paint)
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
    let lut_len = tile.lut.len();
    if lut_len == 0 {
      return Ok(());
    }

    let (ccx, ccy) = (x + tile.cx, y + tile.cy);
    let radius = [(x, y), (x + w, y), (x, y + h), (x + w, y + h)]
      .into_iter()
      .map(|(px, py)| (px - ccx).hypot(py - ccy))
      .fold(0.0_f32, f32::max);

    let group = self.doc.begin_clipped_group(&rect.path_data())?;
    for i in 0..CONIC_WEDGES {
      let a0 = i as f32 / CONIC_WEDGES as f32 * TAU;
      let a1 = (i + 1) as f32 / CONIC_WEDGES as f32 * TAU;
      let mid = (a0 + a1) / 2.0;
      let adjusted = (mid - tile.start_rad).rem_euclid(TAU);
      let idx = tile.lut_index_for_adjusted_angle_with_len(adjusted, lut_len);
      let fill = Rgba::demultiplied(tile.lut.sample(idx));
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
      self
        .doc
        .fill_path(&wedge.into_string(), fill, FillRule::NonZero)?;
    }
    self.doc.end_group(group)
  }
}

fn svg_stops(stops: Vec<SrgbStop>) -> Vec<GradientStop> {
  stops
    .into_iter()
    .map(|stop| GradientStop {
      offset: stop.offset,
      color: Rgba(stop.color.0),
    })
    .collect()
}
