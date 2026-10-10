use std::sync::Arc;

use smallvec::SmallVec;
use takumi_core::{
  geometry::{ComputedLayout as Layout, Point, Rect, Size},
  layout::background_image_geometry::{
    BackgroundLayer, BoxBackgroundPaintContext, FillLayers, ImageTiling,
  },
  paint::{ConicGradientTile, GradientOverlayTile, LinearGradientTile, RadialGradientTile},
  painter::{BoxBackground, SnappedBox},
};
use tiny_skia::{
  FillRule as TinyFillRule, IntSize, Mask as TinyMask, PathBuilder, Pixmap, PixmapMut, PixmapRef,
  PremultipliedColorU8, Rect as TinyRect, Transform as TinyTransform,
};

#[cfg(feature = "svg")]
use crate::pixmap_from_buffer;
use crate::resources::image::RenderedImage;
use crate::{
  BilinearAxis, BorderProperties, DrawTarget, MaskView, OverlayOptions, PaintSource, RenderContext,
  Result, RowSource, SamplingFootprint, checked_area, interpolate_with_footprint,
  layout::node::resolve_image,
  overlay_image, pixmap_ref_from_buffer,
  resources::{image::ImageSource, image_buffer::ImageBuffer},
  style::*,
  try_overlay_gradient_tile,
};

/// One layer's rendered tile and where its copies land, relative to the border box.
pub(crate) struct TileLayer {
  pub blend_mode: BlendMode,
  pub tile: BackgroundTile,
  pub xs: SmallVec<[f32; 1]>,
  pub ys: SmallVec<[f32; 1]>,
  /// The whole-pixel rectangle the tiles show in, when it is less than the painted area.
  pub dest: Option<Rect<f32>>,
}

impl TileLayer {
  /// A layer of one `tile` at `at`, drawn at its own size and unclipped.
  pub(crate) fn whole(tile: BackgroundTile, at: Point<f32>, blend_mode: BlendMode) -> Self {
    Self {
      blend_mode,
      tile,
      xs: [at.x].into(),
      ys: [at.y].into(),
      dest: None,
    }
  }

  /// Whether every tile draws on whole pixels and unclipped, so it can blit.
  pub(crate) fn blits(&self) -> bool {
    self.dest.is_none()
      && self
        .xs
        .iter()
        .chain(&self.ys)
        .all(|&origin| whole_pixel(origin).is_some())
  }

  /// Where the tile at `(x, y)` draws under `transform`, a translation within float error of whole
  /// pixels landing on them.
  pub(crate) fn tile_transform(&self, transform: Affine, x: f32, y: f32) -> Affine {
    let mut placed = transform * Affine::translation(x, y);

    if placed.only_translation()
      && let (Some(x), Some(y)) = (whole_pixel(placed.x), whole_pixel(placed.y))
    {
      placed.x = x;
      placed.y = y;
    }

    placed
  }

  /// A mask over a `size` pixmap, `offset` from the border box, keeping only `dest`.
  fn dest_mask(&self, size: Size<u32>, offset: Point<f32>) -> Option<TinyMask> {
    let dest = self.dest?;
    let mut mask = TinyMask::new(size.width, size.height)?;
    let rect = TinyRect::from_ltrb(
      dest.left + offset.x,
      dest.top + offset.y,
      dest.right + offset.x,
      dest.bottom + offset.y,
    )?;

    mask.fill_path(
      &PathBuilder::from_rect(rect),
      TinyFillRule::Winding,
      false,
      TinyTransform::identity(),
    );
    Some(mask)
  }
}

pub(crate) type TileLayers = Vec<TileLayer>;

/// The whole pixel `value` lies on, within the float error of the sums that place a tile.
pub(crate) fn whole_pixel(value: f32) -> Option<f32> {
  let rounded = value.round();

  ((value - rounded).abs() < 1e-3).then_some(rounded)
}

fn should_rasterize_repeated_tile(
  tile: &BackgroundTile,
  xs: &SmallVec<[f32; 1]>,
  ys: &SmallVec<[f32; 1]>,
) -> bool {
  xs.len().saturating_mul(ys.len()) > 1
    && matches!(
      tile,
      BackgroundTile::Linear(_)
        | BackgroundTile::Radial(_)
        | BackgroundTile::Conic(_)
        | BackgroundTile::SampledBitmap { .. }
    )
}

fn rasterize_tile(tile: BackgroundTile) -> Result<BackgroundTile> {
  let (width, height) = tile.dimensions();
  let Some(size) = IntSize::from_wh(width, height) else {
    return Ok(tile);
  };
  let Some(len) = checked_area(width, height, 4) else {
    return Ok(tile);
  };
  let mut data = vec![0; len];
  let row_bytes = width as usize * 4;

  let rows = tile.rows(width);

  for (y, dst_row) in data.chunks_exact_mut(row_bytes).enumerate() {
    rows.fill(y as u32, dst_row);
  }

  let Some(pixmap) = Pixmap::from_vec(data, size) else {
    return Ok(tile);
  };
  Ok(BackgroundTile::Pixmap(Arc::new(pixmap)))
}

pub(crate) fn rasterize_layers(
  layers: TileLayers,
  size: Size<u32>,
  context: &RenderContext,
  border: BorderProperties,
  transform: Affine,
) -> Result<Option<BackgroundTile>> {
  if layers.is_empty() || size.width == 0 || size.height == 0 {
    return Ok(None);
  }

  let Some(pixmap_size) = IntSize::from_wh(size.width, size.height) else {
    return Ok(None);
  };
  let Some(composed_len) = checked_area(size.width, size.height, 4) else {
    return Ok(None);
  };
  let mut composed = vec![0; composed_len];
  let Some(mut pixmap) = PixmapMut::from_bytes(&mut composed, size.width, size.height) else {
    return Ok(None);
  };

  for layer in layers {
    let dest_mask = layer.dest_mask(
      size,
      Point {
        x: transform.x,
        y: transform.y,
      },
    );
    let mask_view = || {
      dest_mask.as_ref().map(|mask| MaskView {
        mask,
        origin: Point { x: 0, y: 0 },
        canvas_origin: Point { x: 0, y: 0 },
      })
    };

    for &x in &layer.xs {
      for &y in &layer.ys {
        let layer_transform = layer.tile_transform(transform, x, y);

        if border.is_zero()
          && layer_transform.only_translation()
          && layer_transform.x.fract() == 0.0
          && layer_transform.y.fract() == 0.0
          && layer.blend_mode == BlendMode::Normal
          && try_overlay_gradient_tile(
            &mut pixmap,
            &layer.tile,
            Point {
              x: layer_transform.x,
              y: layer_transform.y,
            },
            layer.blend_mode,
            mask_view(),
          )
        {
          continue;
        }

        overlay_image(
          &mut DrawTarget {
            pixmap: &mut pixmap,
            combined_mask: mask_view(),
          },
          &layer.tile,
          OverlayOptions {
            border,
            transform: layer_transform,
            algorithm: context.style.rare_inherited_data.image_rendering,
            mode: layer.blend_mode,
          },
        );
      }
    }
  }

  let Some(pixmap) = Pixmap::from_vec(composed, pixmap_size) else {
    return Ok(None);
  };
  Ok(Some(BackgroundTile::Pixmap(Arc::new(pixmap))))
}

pub(crate) struct ColorTile {
  color: Color,
  premultiplied: PremultipliedColorU8,
  width: u32,
  height: u32,
}

impl ColorTile {
  pub(crate) fn new(color: Color, width: u32, height: u32) -> Self {
    Self {
      color,
      premultiplied: color.premultiplied(),
      width,
      height,
    }
  }

  pub(crate) fn color(&self) -> Color {
    self.color
  }

  pub(crate) fn width(&self) -> u32 {
    self.width
  }

  pub(crate) fn height(&self) -> u32 {
    self.height
  }

  pub(crate) fn get_pixel(&self, _x: u32, _y: u32) -> PremultipliedColorU8 {
    self.premultiplied
  }
}

/// Pixel-independent state for sampling a bitmap background tile.
#[derive(Clone, Copy)]
pub(crate) struct SampledBitmapView<'a> {
  source: PixmapRef<'a>,
  algorithm: ImageScalingAlgorithm,
  /// The size the tile is drawn at.
  logical_size: Size<u32>,
  footprint: SamplingFootprint,
}

impl<'a> SampledBitmapView<'a> {
  fn new(
    source: &'a ImageBuffer,
    width: u32,
    height: u32,
    algorithm: ImageScalingAlgorithm,
  ) -> Option<Self> {
    let source = pixmap_ref_from_buffer(source)?;
    let logical_size = Size { width, height };

    Some(Self {
      source,
      algorithm,
      logical_size,
      footprint: SamplingFootprint::new(
        source.width() as f32 / logical_size.width.max(1) as f32,
        source.height() as f32 / logical_size.height.max(1) as f32,
      ),
    })
  }

  /// The size the tile is drawn at.
  pub(crate) fn size(&self) -> Size<u32> {
    self.logical_size
  }

  /// The source pixmap when the tile is drawn at exactly the source's size, so destination pixel
  /// `(x, y)` is source pixel `(x, y)`.
  pub(crate) fn identity_source(&self) -> Option<PixmapRef<'a>> {
    (self.source.width() == self.logical_size.width
      && self.source.height() == self.logical_size.height)
      .then_some(self.source)
  }

  /// Keep the `(x + 0.5) * source / logical` form, casts included.
  #[inline]
  pub(crate) fn sample(&self, x: u32, y: u32) -> PremultipliedColorU8 {
    interpolate_with_footprint(
      self.source.into(),
      self.algorithm,
      (x as f32 + 0.5) * self.source.width() as f32 / self.logical_size.width.max(1) as f32,
      (y as f32 + 0.5) * self.source.height() as f32 / self.logical_size.height.max(1) as f32,
      self.footprint,
    )
    .unwrap_or(PremultipliedColorU8::TRANSPARENT)
  }

  /// Column lookups for bilinear sampling of destination columns `x_start..x_start + width`, or
  /// `None` when the tile samples nearest or box.
  pub(crate) fn bilinear_rows(&self, x_start: u32, width: u32) -> Option<BilinearRows<'a>> {
    if matches!(self.algorithm, ImageScalingAlgorithm::Pixelated) || self.footprint.is_minifying() {
      return None;
    }

    let source_width = self.source.width();
    let columns = (x_start..x_start.checked_add(width)?)
      .map(|x| {
        BilinearAxis::new(
          (x as f32 + 0.5) * source_width as f32 / self.logical_size.width.max(1) as f32,
          source_width,
        )
      })
      .collect();

    Some(BilinearRows {
      source: self.source,
      logical_height: self.logical_size.height,
      columns,
    })
  }
}

/// Bilinear sampling of a scaled tile one destination row at a time.
pub(crate) struct BilinearRows<'a> {
  source: PixmapRef<'a>,
  logical_height: u32,
  columns: Vec<BilinearAxis>,
}

impl BilinearRows<'_> {
  pub(crate) fn fill(&self, y: u32, dst: &mut [[u8; 4]]) {
    let source_height = self.source.height();
    let row = BilinearAxis::new(
      (y as f32 + 0.5) * source_height as f32 / self.logical_height.max(1) as f32,
      source_height,
    )
    .rows(self.source);

    for (out, column) in dst.iter_mut().zip(&self.columns) {
      *out = column.mix(row);
    }
  }
}

pub(crate) enum BackgroundTile {
  Linear(LinearGradientTile),
  Radial(RadialGradientTile),
  Conic(ConicGradientTile),
  Pixmap(Arc<Pixmap>),
  SampledBitmap {
    source: Arc<ImageBuffer>,
    width: u32,
    height: u32,
    algo: ImageScalingAlgorithm,
  },
  Color(ColorTile),
}

/// Row producer for a tile, resolved once so a full rasterization reuses its state.
pub(crate) enum TileRows<'a> {
  Linear(&'a LinearGradientTile),
  Radial(&'a RadialGradientTile),
  Conic(&'a ConicGradientTile),
  Source(RowSource<'a>),
}

impl TileRows<'_> {
  pub(crate) fn fill(&self, y: u32, dst: &mut [u8]) {
    let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(dst);

    match self {
      Self::Linear(t) => fill_gradient_row(*t, y, pixels),
      Self::Radial(t) => fill_gradient_row(*t, y, pixels),
      Self::Conic(t) => fill_gradient_row(*t, y, pixels),
      Self::Source(rows) => rows.fill(y, pixels),
    }
  }
}

fn fill_gradient_row<T: GradientOverlayTile>(t: &T, y: u32, pixels: &mut [[u8; 4]]) {
  let lut_len = t.lut_len();
  let mut row_state = t.begin_row(0, y, lut_len);
  let dither = t.dither_active();
  for (x, chunk) in pixels.iter_mut().enumerate() {
    let lut_idx = t.next_lut_index(&mut row_state);
    let p = if dither {
      t.sample_dithered_at(lut_idx, x as u32, y)
    } else {
      t.sample_at(lut_idx)
    };
    *chunk = [p.red(), p.green(), p.blue(), p.alpha()];
  }
}

impl BackgroundTile {
  pub(crate) fn width(&self) -> u32 {
    match self {
      Self::Linear(t) => t.width(),
      Self::Radial(t) => t.width(),
      Self::Conic(t) => t.width(),
      Self::Pixmap(t) => t.width(),
      Self::SampledBitmap { width, .. } => *width,
      Self::Color(t) => t.width(),
    }
  }

  pub(crate) fn height(&self) -> u32 {
    match self {
      Self::Linear(t) => t.height(),
      Self::Radial(t) => t.height(),
      Self::Conic(t) => t.height(),
      Self::Pixmap(t) => t.height(),
      Self::SampledBitmap { height, .. } => *height,
      Self::Color(t) => t.height(),
    }
  }

  pub(crate) fn dimensions(&self) -> (u32, u32) {
    (self.width(), self.height())
  }

  pub(crate) fn get_pixel(&self, x: u32, y: u32) -> PremultipliedColorU8 {
    match self {
      Self::Linear(t) => t.sample_pixel(x, y),
      Self::Radial(t) => t.sample_pixel(x, y),
      Self::Conic(t) => t.sample_pixel(x, y),
      Self::Pixmap(t) => PaintSource::from(t.as_ref()).get_pixel(x, y),
      Self::SampledBitmap { .. } => self
        .sampled_bitmap_view()
        .map_or(PremultipliedColorU8::TRANSPARENT, |view| view.sample(x, y)),
      Self::Color(t) => t.get_pixel(x, y),
    }
  }

  /// Hoisted sampling state for a [`Self::SampledBitmap`].
  pub(crate) fn sampled_bitmap_view(&self) -> Option<SampledBitmapView<'_>> {
    let Self::SampledBitmap {
      source,
      width,
      height,
      algo,
    } = self
    else {
      return None;
    };

    SampledBitmapView::new(source.as_ref(), *width, *height, *algo)
  }

  pub(crate) fn rows(&self, width: u32) -> TileRows<'_> {
    match self {
      Self::Linear(t) => TileRows::Linear(t),
      Self::Radial(t) => TileRows::Radial(t),
      Self::Conic(t) => TileRows::Conic(t),
      _ => TileRows::Source(PaintSource::from(self).rows(0, width)),
    }
  }

  pub(crate) fn as_raw(&self) -> Option<&[u8]> {
    match self {
      Self::Pixmap(pixmap) => Some(pixmap.data()),
      _ => None,
    }
  }
}

/// Builds one tile of a background layer at the size the geometry resolved to.
pub(crate) fn render_tile(
  image: &BackgroundImage,
  tile_w: u32,
  tile_h: u32,
  context: &RenderContext,
) -> Result<Option<BackgroundTile>> {
  Ok(match image {
    BackgroundImage::None => None,
    BackgroundImage::Linear(gradient) => Some(BackgroundTile::Linear(LinearGradientTile::new(
      gradient,
      tile_w,
      tile_h,
      &context.sizing,
      context.current_color,
      context.dither_gradients(),
    ))),
    BackgroundImage::Radial(gradient) => Some(BackgroundTile::Radial(RadialGradientTile::new(
      gradient,
      tile_w,
      tile_h,
      &context.sizing,
      context.current_color,
      context.dither_gradients(),
    ))),
    BackgroundImage::Conic(gradient) => Some(BackgroundTile::Conic(ConicGradientTile::new(
      gradient,
      tile_w,
      tile_h,
      &context.sizing,
      context.current_color,
      context.dither_gradients(),
    ))),
    BackgroundImage::Url(url) => {
      let Ok(source) = resolve_image(url, context) else {
        return Ok(None);
      };

      match &source {
        ImageSource::Bitmap(bitmap) => Some(BackgroundTile::SampledBitmap {
          source: bitmap.clone(),
          width: tile_w,
          height: tile_h,
          algo: context.style.rare_inherited_data.image_rendering,
        }),
        #[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
        ImageSource::Animated(animated) => Some(BackgroundTile::SampledBitmap {
          source: animated.frame_at_time_covering(
            context.time_ms(),
            tile_w,
            tile_h,
            context.style.rare_inherited_data.image_rendering,
          ),
          width: tile_w,
          height: tile_h,
          algo: context.style.rare_inherited_data.image_rendering,
        }),
        ImageSource::Encoded(..) => match source.render_for_layout(
          tile_w,
          tile_h,
          context.style.rare_inherited_data.image_rendering,
          context.time_ms(),
          context.current_color,
          Some(context.fonts()),
        )? {
          RenderedImage::Sampled { source, .. } => Some(BackgroundTile::SampledBitmap {
            source,
            width: tile_w,
            height: tile_h,
            algo: context.style.rare_inherited_data.image_rendering,
          }),
          RenderedImage::Rasterized(..) => None,
        },
        #[cfg(feature = "svg")]
        ImageSource::Svg(..) => match source.render_for_layout(
          tile_w,
          tile_h,
          context.style.rare_inherited_data.image_rendering,
          context.time_ms(),
          context.current_color,
          Some(context.fonts()),
        )? {
          RenderedImage::Rasterized(buffer) => {
            pixmap_from_buffer(&buffer).map(|pixmap| BackgroundTile::Pixmap(Arc::new(pixmap)))
          }
          RenderedImage::Sampled { .. } => None,
        },
        _ => None,
      }
    }
  })
}

/// Renders each layer's tile and places its copies over `paint`, a rectangle in border-box space.
pub(crate) fn tile_layers(
  layers: &[BackgroundLayer<'_>],
  paint: Rect<f32>,
  context: &RenderContext,
) -> Result<TileLayers> {
  let mut resolved = Vec::with_capacity(layers.len());

  for layer in layers {
    let tiling = layer.tiling;

    if !lands_on_pixels(&tiling) {
      if let Some(pattern) = rasterize_pattern(layer.image, &tiling, context)? {
        resolved.push(TileLayer::whole(
          BackgroundTile::Pixmap(Arc::new(pattern)),
          tiling.dest.top_left(),
          layer.blend_mode,
        ));
      }
      continue;
    }

    let Some(tile) = render_tile(
      layer.image,
      tiling.tile.width as u32,
      tiling.tile.height as u32,
      context,
    )?
    else {
      continue;
    };
    let (xs, ys) = tiling.origins();
    let tile = if should_rasterize_repeated_tile(&tile, &xs, &ys) {
      rasterize_tile(tile)?
    } else {
      tile
    };
    resolved.push(TileLayer {
      tile,
      xs,
      ys,
      dest: (!tiling.covers(paint)).then_some(tiling.dest),
      blend_mode: layer.blend_mode,
    });
  }

  Ok(resolved)
}

/// Whether every tile of `tiling` covers whole pixels of its `dest`, so a tile rendered on its own
/// pixel grid blits.
fn lands_on_pixels(tiling: &ImageTiling) -> bool {
  let whole = |value: f32| whole_pixel(value).is_some();

  whole(tiling.tile.width)
    && whole(tiling.tile.height)
    && whole(tiling.spacing.width)
    && whole(tiling.spacing.height)
    && whole(tiling.phase.x - tiling.dest.left)
    && whole(tiling.phase.y - tiling.dest.top)
}

/// Paints `image` over `tiling`'s `dest` as Skia's shaders paint a tiled background: each pixel
/// centre finds its point in the tile it lands in and samples the image there, a bitmap from its
/// source, and a gradient or an SVG from its rasterization at the tile's whole-pixel size, as
/// Blink's `GeneratedImage::DrawPattern` records the tile for Skia's picture shader.
fn rasterize_pattern(
  image: &BackgroundImage,
  tiling: &ImageTiling,
  context: &RenderContext,
) -> Result<Option<Pixmap>> {
  let dest = tiling.dest;
  let width = (dest.right - dest.left).round() as u32;
  let height = (dest.bottom - dest.top).round() as u32;
  let Some(size) = IntSize::from_wh(width, height) else {
    return Ok(None);
  };
  let Some(len) = checked_area(width, height, 4) else {
    return Ok(None);
  };
  let dither = context.dither_gradients();
  let picture = match image {
    BackgroundImage::Linear(gradient) => Some(picture_tile(
      &LinearGradientTile::sized(
        gradient,
        tiling.tile,
        &context.sizing,
        context.current_color,
        dither,
      ),
      tiling.tile,
    )),
    BackgroundImage::Radial(gradient) => Some(picture_tile(
      &RadialGradientTile::sized(
        gradient,
        tiling.tile,
        &context.sizing,
        context.current_color,
        dither,
      ),
      tiling.tile,
    )),
    BackgroundImage::Conic(gradient) => Some(picture_tile(
      &ConicGradientTile::sized(
        gradient,
        tiling.tile,
        &context.sizing,
        context.current_color,
        dither,
      ),
      tiling.tile,
    )),
    _ => None,
  };
  let content = match (picture, image) {
    (Some(picture), _) => {
      let Some(picture) = picture else {
        return Ok(None);
      };

      BackgroundTile::Pixmap(Arc::new(picture))
    }
    (None, image) => {
      let Some(tile) = render_tile(
        image,
        tiling.tile.width.ceil() as u32,
        tiling.tile.height.ceil() as u32,
        context,
      )?
      else {
        return Ok(None);
      };
      tile
    }
  };
  let source = match &content {
    BackgroundTile::SampledBitmap { source, algo, .. } => {
      pixmap_ref_from_buffer(source).map(|source| (source, *algo))
    }
    BackgroundTile::Pixmap(pixmap) => Some((
      pixmap.as_ref().as_ref(),
      context.style.rare_inherited_data.image_rendering,
    )),
    _ => None,
  };
  let source_per_tile = source.map(|(source, _)| Size {
    width: source.width() as f32 / tiling.tile.width,
    height: source.height() as f32 / tiling.tile.height,
  });
  let padded = source.and_then(|(source, _)| padded_tile(source, tiling.spacing));
  let source = source
    .zip(padded.as_ref())
    .map(|((_, algorithm), padded)| (padded.as_ref(), algorithm));
  let step = tiling.step();
  let mut data = vec![0; len];

  for (j, row) in data.chunks_exact_mut(width as usize * 4).enumerate() {
    let v = (dest.top + j as f32 + 0.5 - tiling.phase.y).rem_euclid(step.height);

    if v >= tiling.tile.height {
      continue;
    }
    for (i, pixel) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
      let u = (dest.left + i as f32 + 0.5 - tiling.phase.x).rem_euclid(step.width);

      if u >= tiling.tile.width {
        continue;
      }
      let color = match (&content, source, source_per_tile) {
        (BackgroundTile::Color(tile), ..) => tile.get_pixel(0, 0),
        (_, Some((source, algorithm)), Some(scale)) => interpolate_with_footprint(
          source.into(),
          algorithm,
          u * scale.width + 1.0,
          v * scale.height + 1.0,
          SamplingFootprint::new(scale.width, scale.height),
        )
        .unwrap_or(PremultipliedColorU8::TRANSPARENT),
        _ => PremultipliedColorU8::TRANSPARENT,
      };

      *pixel = [color.red(), color.green(), color.blue(), color.alpha()];
    }
  }

  Ok(Pixmap::from_vec(data, size))
}

/// `source` inside a one-pixel border of what its neighbours show there: the next tile's opposite
/// edge where tiles abut, as Skia's repeat tiling filters across the seam, and nothing across
/// `spacing`.
fn padded_tile(source: PixmapRef<'_>, spacing: Size<f32>) -> Option<Pixmap> {
  let (width, height) = (source.width() as usize, source.height() as usize);
  let mut padded = Pixmap::new(width as u32 + 2, height as u32 + 2)?;
  let pixels = source.pixels();
  let target = padded.pixels_mut();
  let wrap = |index: usize, len: usize, gap: f32| match index {
    0 if gap == 0.0 => Some(len - 1),
    0 => None,
    index if index == len + 1 && gap == 0.0 => Some(0),
    index if index == len + 1 => None,
    index => Some(index - 1),
  };

  for y in 0..height + 2 {
    let Some(source_y) = wrap(y, height, spacing.height) else {
      continue;
    };

    for x in 0..width + 2 {
      if let Some(source_x) = wrap(x, width, spacing.width) {
        target[y * (width + 2) + x] = pixels[source_y * width + source_x];
      }
    }
  }

  Some(padded)
}

/// `gradient`'s `tile` rasterized as Skia's picture shader rasterizes a recorded tile: into a
/// bitmap of whole pixels with the tile stretched to fill it.
fn picture_tile(gradient: &impl GradientOverlayTile, tile: Size<f32>) -> Option<Pixmap> {
  let (width, height) = (tile.width.ceil() as u32, tile.height.ceil() as u32);
  let (scale_x, scale_y) = (tile.width / width as f32, tile.height / height as f32);
  let mut data = vec![0; checked_area(width, height, 4)?];

  for (j, row) in data.chunks_exact_mut(width as usize * 4).enumerate() {
    for (i, pixel) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
      let color = gradient.sample_point(
        (i as f32 + 0.5) * scale_x,
        (j as f32 + 0.5) * scale_y,
        (i as u32, j as u32),
      );

      *pixel = [color.red(), color.green(), color.blue(), color.alpha()];
    }
  }

  Pixmap::from_vec(data, IntSize::from_wh(width, height)?)
}

/// A box's `mask-image` alpha over its snapped border box, `offset` from the border box.
pub(crate) struct BoxMask {
  pub(crate) alpha: Vec<u8>,
  pub(crate) offset: Point<f32>,
  pub(crate) size: Size<u32>,
}

pub(crate) fn create_mask(context: &RenderContext, layout: Layout) -> Result<Option<BoxMask>> {
  let paint_offset = context.box_paint_offset(layout);
  let snapped = SnappedBox::new(paint_offset, layout.size);
  let offset = snapped.offset();
  let size = snapped.size().map(|x| x as u32);
  let layers = tile_layers(
    &FillLayers::mask(&context.style).resolve(
      context
        .style
        .rare_non_inherited_data
        .mask_image
        .as_deref()
        .unwrap_or(&[]),
      &BoxBackgroundPaintContext::mask(layout.size, paint_offset),
      context,
    ),
    Rect {
      left: offset.x,
      top: offset.y,
      right: offset.x + snapped.size().width,
      bottom: offset.y + snapped.size().height,
    },
    context,
  )?;

  if layers.is_empty() {
    return Ok(None);
  }

  let empty = BoxMask {
    alpha: Vec::new(),
    offset,
    size,
  };
  // An empty mask hides the node. A mask this size cannot be rasterized, and
  // dropping it would paint the node unmasked instead.
  let Some(tile) = rasterize_layers(
    layers,
    size,
    context,
    BorderProperties::default(),
    Affine::translation(-offset.x, -offset.y),
  )?
  else {
    return Ok(Some(empty));
  };

  let (width, height) = tile.dimensions();
  let Some(len) = checked_area(width, height, 1) else {
    return Ok(Some(empty));
  };
  let mut alpha = vec![0; len];

  if let Some(raw) = tile.as_raw() {
    for (alpha, pixel) in alpha.iter_mut().zip(raw.as_chunks::<4>().0) {
      *alpha = pixel[3];
    }
  } else {
    let pixels = (0..height).flat_map(|y| (0..width).map(move |x| (x, y)));

    for (alpha, (x, y)) in alpha.iter_mut().zip(pixels) {
      *alpha = tile.get_pixel(x, y).alpha();
    }
  }

  Ok(Some(BoxMask {
    alpha,
    offset,
    size,
  }))
}

/// The `background-image` layers only.
pub(crate) fn background_image_layers(
  background: &BoxBackground<'_>,
  context: &RenderContext,
) -> Result<TileLayers> {
  tile_layers(
    &background.layers,
    Rect {
      left: background.offset.x,
      top: background.offset.y,
      right: background.offset.x + background.size.width,
      bottom: background.offset.y + background.size.height,
    },
    context,
  )
}

/// The `background-image` layers under a `background-color` layer.
pub(crate) fn collect_background_layers(
  background: &BoxBackground<'_>,
  context: &RenderContext,
) -> Result<TileLayers> {
  let mut layers = background_image_layers(background, context)?;

  if let Some(color) = background.color {
    layers.insert(
      0,
      TileLayer::whole(
        BackgroundTile::Color(ColorTile::new(
          color,
          background.size.width as u32,
          background.size.height as u32,
        )),
        background.offset,
        BlendMode::Normal,
      ),
    );
  }

  Ok(layers)
}

#[cfg(test)]
mod tests {
  use std::{collections::HashMap, sync::Arc};

  use super::*;
  use crate::{
    Fonts, RenderOptions,
    layout::node::Node,
    render,
    resources::image::ImageSource,
    style::{
      BackgroundImages, BackgroundRepeats, BackgroundSizes, FromCssStr, ImageScalingAlgorithm,
      Length::Percentage, Style, StyleDeclaration,
    },
    viewport::Viewport,
  };

  const BITMAP_URL: &str = "test://bitmap";

  #[test]
  fn repeated_gradient_tiles_dither_when_active() {
    use crate::{
      RenderContext,
      style::{FromCssStr, LinearGradient, SizingContext},
    };

    let gradient =
      LinearGradient::from_css_str("linear-gradient(to right, #101010, #131313)").unwrap();
    let fonts = Fonts::default();
    let render_context = RenderContext::builder()
      .fonts(fonts.snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((64, 16)))
          .build(),
      )
      .build();
    let make_row = |dither: bool| {
      let tile = BackgroundTile::Linear(LinearGradientTile::new(
        &gradient,
        64,
        16,
        &render_context.sizing,
        render_context.current_color,
        dither,
      ));
      let tile_rows = tile.rows(64);
      let mut rows = Vec::new();
      for y in [0, 1] {
        let mut row = vec![0u8; 64 * 4];
        tile_rows.fill(y, &mut row);
        rows.push(row);
      }
      rows
    };

    let plain = make_row(false);
    assert_eq!(plain[0], plain[1]);

    let dithered = make_row(true);
    assert_ne!(dithered[0], dithered[1]);
  }

  /// Opaque so the premultiplied round-trip through the canvas is lossless and the rendered bytes
  /// can be compared against the source directly.
  fn opaque_source(width: u32, height: u32) -> (Vec<u8>, ImageSource) {
    let mut data = Vec::with_capacity((width as usize) * (height as usize) * 4);
    for y in 0..height {
      for x in 0..width {
        data.extend_from_slice(&[
          (x * 7 % 256) as u8,
          (y * 11 % 256) as u8,
          ((x + y) * 13 % 256) as u8,
          u8::MAX,
        ]);
      }
    }

    let source = ImageBuffer::from_rgba_bytes(data.clone(), width, height)
      .map(ImageSource::from)
      .expect("source buffer dimensions");
    (data, source)
  }

  fn render_background(
    source: ImageSource,
    viewport: (u32, u32),
    algorithm: ImageScalingAlgorithm,
  ) -> Vec<u8> {
    let fonts = Fonts::default();
    let node = Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::width(Percentage(100.0)))
        .with(StyleDeclaration::height(Percentage(100.0)))
        .with(StyleDeclaration::background_image(Some(
          BackgroundImages::from_css_str(&format!("url({BITMAP_URL})")).expect("background url"),
        )))
        .with(StyleDeclaration::background_size(
          BackgroundSizes::from_css_str("100% 100%").expect("background size"),
        ))
        .with(StyleDeclaration::background_repeat(
          BackgroundRepeats::from_css_str("no-repeat").expect("background repeat"),
        ))
        .with(StyleDeclaration::image_rendering(algorithm)),
    );

    let options = RenderOptions::builder()
      .fonts(&fonts)
      .viewport(Viewport::new(viewport))
      .node(node)
      .images(HashMap::from([(Arc::from(BITMAP_URL), source)]))
      .build();

    render(options).expect("render background").into_raw()
  }

  /// A background drawn at the source's own size is a copy of the source, so every
  /// `image-rendering` value has to agree with it and with each other.
  #[test]
  fn one_to_one_background_copies_the_source_for_every_algorithm() {
    let (expected, source) = opaque_source(64, 48);

    let auto = render_background(source.clone(), (64, 48), ImageScalingAlgorithm::Auto);
    let smooth = render_background(source.clone(), (64, 48), ImageScalingAlgorithm::Smooth);
    let pixelated = render_background(source, (64, 48), ImageScalingAlgorithm::Pixelated);

    assert_eq!(auto, expected);
    assert_eq!(smooth, expected);
    assert_eq!(pixelated, expected);
  }

  /// The fast path must not swallow `image-rendering` for a genuinely scaled background: nearest
  /// and the smooth samplers disagree there.
  #[test]
  fn scaled_background_still_honours_image_rendering() {
    let (_, source) = opaque_source(64, 48);

    let auto = render_background(source.clone(), (32, 24), ImageScalingAlgorithm::Auto);
    let pixelated = render_background(source, (32, 24), ImageScalingAlgorithm::Pixelated);

    assert_ne!(auto, pixelated);
  }
}
