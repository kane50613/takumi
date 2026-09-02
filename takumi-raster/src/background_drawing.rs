use std::sync::Arc;

use image::Rgba;
use smallvec::SmallVec;
use takumi_core::{
  geometry::{ComputedLayout as Layout, Point, Size},
  layout::background::{BackgroundLayersInput, background_origin_box},
  paint::{ConicGradientTile, GradientOverlayTile, LinearGradientTile, RadialGradientTile},
};
use tiny_skia::{IntSize, Pixmap, PixmapMut, PixmapRef, PremultipliedColorU8};

#[cfg(feature = "svg")]
use crate::resources::image::RenderedImage;
use crate::{
  BorderProperties, DrawTarget, OverlayOptions, PaintSource, RenderContext, Result,
  SamplingFootprint, checked_area, color_to_premultiplied, interpolate_with_footprint,
  layout::node::resolve_image,
  overlay_image, pixmap_from_buffer, pixmap_ref_from_buffer,
  resources::{image::ImageSource, image_buffer::ImageBuffer},
  style::*,
  try_overlay_gradient_tile,
};

pub(crate) struct TileLayer {
  pub blend_mode: BlendMode,
  pub tile: BackgroundTile,
  pub xs: SmallVec<[i32; 1]>,
  pub ys: SmallVec<[i32; 1]>,
}

pub(crate) type TileLayers = Vec<TileLayer>;

fn should_rasterize_repeated_tile(
  tile: &BackgroundTile,
  xs: &SmallVec<[i32; 1]>,
  ys: &SmallVec<[i32; 1]>,
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

  for y in 0..height {
    let row_offset = y as usize * row_bytes;
    let dst_row = &mut data[row_offset..row_offset + row_bytes];
    tile.rasterize_row(y, width, dst_row);
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
    for &x in &layer.xs {
      for &y in &layer.ys {
        let layer_transform = Affine::translation(x as f32, y as f32) * transform;
        if border.is_zero()
          && layer_transform.only_translation()
          && layer.blend_mode == BlendMode::Normal
          && try_overlay_gradient_tile(
            &mut pixmap,
            &layer.tile,
            Point {
              x: layer_transform.x,
              y: layer_transform.y,
            },
            layer.blend_mode,
            None,
          )
        {
          continue;
        }

        overlay_image(
          &mut DrawTarget {
            pixmap: &mut pixmap,
            combined_mask: None,
          },
          &layer.tile,
          OverlayOptions {
            border,
            transform: layer_transform,
            algorithm: context.style.image_rendering,
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
  pub width: u32,
  pub height: u32,
}

impl ColorTile {
  pub(crate) fn new(color: Color, width: u32, height: u32) -> Self {
    Self {
      color,
      premultiplied: color_to_premultiplied(Rgba(color.0)),
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

/// Everything about sampling a [`BackgroundTile::SampledBitmap`] that does not
/// depend on the pixel being sampled: the source view, the size the mapping
/// divides by, and the footprint. Resolved once per tile, not per pixel.
#[derive(Clone, Copy)]
pub(crate) struct SampledBitmapView<'a> {
  source: PixmapRef<'a>,
  algorithm: ImageScalingAlgorithm,
  /// The size the tile is drawn at. When it equals the source's own size the
  /// mapping is the identity.
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

  /// The source pixmap when the tile is drawn at exactly the source's size, so
  /// destination pixel `(x, y)` is source pixel `(x, y)`.
  ///
  /// Every sampler agrees there. The sample lands on `x + 0.5`, which nearest
  /// floors back to `x` and bilinear resolves to the same texel at zero
  /// interpolation weight, and a one-pixel footprint never minifies. So the
  /// scaling algorithm can be ignored and the pixels copied as they are.
  pub(crate) fn identity_source(&self) -> Option<PixmapRef<'a>> {
    (self.source.width() == self.logical_size.width
      && self.source.height() == self.logical_size.height)
      .then_some(self.source)
  }

  /// Keep the `(x + 0.5) * source / logical` form, casts included. Folding the
  /// division into a precomputed scale rounds differently and shifts which texel
  /// a minified sample lands on.
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

  /// Copies source row `y` into `dst` when the tile is drawn 1:1, reporting
  /// whether it did. A mismatched width would misalign the row offsets, so it
  /// falls back to sampling.
  fn try_copy_row(&self, y: u32, dst: &mut [[u8; 4]]) -> bool {
    let Some(source) = self.identity_source() else {
      return false;
    };
    if source.width() as usize != dst.len() {
      return false;
    }

    let stride = dst.len() * 4;
    let start = y as usize * stride;
    let Some(row) = source.data().get(start..start + stride) else {
      return false;
    };

    dst.copy_from_slice(bytemuck::cast_slice(row));
    true
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

  /// The hoisted sampling state for a [`Self::SampledBitmap`], so a caller that
  /// walks many pixels resolves the source once instead of per pixel.
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

  pub(crate) fn rasterize_row(&self, y: u32, width: u32, dst: &mut [u8]) {
    debug_assert_eq!(dst.len(), (width * 4) as usize);
    let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(dst);

    fn rasterize_gradient_row<T: GradientOverlayTile>(t: &T, y: u32, pixels: &mut [[u8; 4]]) {
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

    match self {
      Self::Linear(t) => rasterize_gradient_row(t, y, pixels),
      Self::Radial(t) => rasterize_gradient_row(t, y, pixels),
      Self::Conic(t) => rasterize_gradient_row(t, y, pixels),
      Self::Pixmap(t) => {
        let ps = PaintSource::from(t.as_ref());
        for (x, chunk) in pixels.iter_mut().enumerate() {
          let p = ps.get_pixel(x as u32, y);
          *chunk = [p.red(), p.green(), p.blue(), p.alpha()];
        }
      }
      Self::SampledBitmap { .. } => {
        let Some(view) = self.sampled_bitmap_view() else {
          pixels.fill([0; 4]);
          return;
        };

        if view.try_copy_row(y, pixels) {
          return;
        }

        for (x, chunk) in pixels.iter_mut().enumerate() {
          let p = view.sample(x as u32, y);
          *chunk = [p.red(), p.green(), p.blue(), p.alpha()];
        }
      }
      Self::Color(t) => {
        let p = t.get_pixel(0, 0);
        let bytes = [p.red(), p.green(), p.blue(), p.alpha()];
        for chunk in pixels.iter_mut() {
          *chunk = bytes;
        }
      }
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
      if let Ok(source) = resolve_image(url, context) {
        match &source {
          ImageSource::Bitmap(bitmap) => Some(BackgroundTile::SampledBitmap {
            source: bitmap.clone(),
            width: tile_w,
            height: tile_h,
            algo: context.style.image_rendering,
          }),
          ImageSource::Animated(animated) => Some(BackgroundTile::SampledBitmap {
            source: animated.frame_at_time_covering(
              context.time_ms(),
              tile_w,
              tile_h,
              context.style.image_rendering,
            ),
            width: tile_w,
            height: tile_h,
            algo: context.style.image_rendering,
          }),
          ImageSource::Encoded(..) => match source.render_for_layout(
            tile_w,
            tile_h,
            context.style.image_rendering,
            context.time_ms(),
            context.current_color,
            Some(context.fonts()),
          )? {
            RenderedImage::Sampled { source, .. } => Some(BackgroundTile::SampledBitmap {
              source,
              width: tile_w,
              height: tile_h,
              algo: context.style.image_rendering,
            }),
            RenderedImage::Rasterized(..) => None,
          },
          #[cfg(feature = "svg")]
          ImageSource::Svg(..) => match source.render_for_layout(
            tile_w,
            tile_h,
            context.style.image_rendering,
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
      } else {
        None
      }
    }
  })
}

/// Resolve tile image, positions along X and Y for a background-like layer.
pub(crate) fn resolve_tile_layers(input: BackgroundLayersInput<'_>) -> Result<TileLayers> {
  let images = input.images;
  let context = input.context;
  let mut layers = Vec::new();

  for (index, geometry) in input.resolve() {
    let Some(image) = images.get(index) else {
      continue;
    };
    let Some(tile) = render_tile(image, geometry.tile_width, geometry.tile_height, context)? else {
      continue;
    };
    let tile = if should_rasterize_repeated_tile(&tile, &geometry.xs, &geometry.ys) {
      rasterize_tile(tile)?
    } else {
      tile
    };

    layers.push(TileLayer {
      tile,
      xs: geometry.xs,
      ys: geometry.ys,
      blend_mode: geometry.blend_mode,
    });
  }

  Ok(layers)
}

pub(crate) fn create_mask(
  context: &RenderContext,
  border_box: Size<f32>,
) -> Result<Option<Vec<u8>>> {
  let mask_image = context.style.mask_image.as_deref().unwrap_or(&[]);
  let mask_position = context.style.mask_position.as_ref();
  let mask_size = context.style.mask_size.as_ref();
  let mask_repeat = context.style.mask_repeat.as_ref();

  let layers = resolve_tile_layers(BackgroundLayersInput {
    images: mask_image,
    positions: mask_position,
    sizes: mask_size,
    repeats: mask_repeat,
    blend_modes: &[], // no blending mode for mask
    context,
    area: border_box.map(|x| x as u32),
    paint: border_box.map(|x| x as u32),
    origin_offset: Point { x: 0, y: 0 },
  })?;

  if layers.is_empty() {
    return Ok(None);
  }

  // An empty mask hides the node. A mask this size cannot be rasterized, and
  // dropping it would paint the node unmasked instead.
  let size = border_box.map(|x| x as u32);
  let Some(tile) = rasterize_layers(
    layers,
    size,
    context,
    BorderProperties::default(),
    Affine::IDENTITY,
  )?
  else {
    return Ok(Some(Vec::new()));
  };

  Ok(Some({
    let (w, h) = tile.dimensions();
    let Some(len) = checked_area(w, h, 1) else {
      return Ok(Some(Vec::new()));
    };
    let mut alpha = vec![0; len];

    if let Some(raw) = tile.as_raw() {
      let count = alpha.len().min(raw.len() / 4);
      for i in 0..count {
        alpha[i] = raw[i * 4 + 3];
      }
      for alpha_val in alpha.iter_mut().skip(count) {
        *alpha_val = 0;
      }
    } else {
      let mut i = 0;
      for y in 0..h {
        for x in 0..w {
          if i < alpha.len() {
            alpha[i] = tile.get_pixel(x, y).alpha();
            i += 1;
          }
        }
      }
      for alpha_val in alpha.iter_mut().skip(i) {
        *alpha_val = 0;
      }
    }

    alpha
  }))
}

/// The `background-image` layers only. The colour underneath them is painted
/// through [`crate::node_paint::CanvasDevice`], so a caller that paints it
/// itself asks for this instead of [`collect_background_layers`].
pub(crate) fn background_image_layers(
  context: &RenderContext,
  layout: Layout,
) -> Result<TileLayers> {
  let border_box = layout.size;
  let origin = background_origin_box(context.style.background_origin, layout);

  resolve_tile_layers(BackgroundLayersInput {
    images: context.style.background_image.as_deref().unwrap_or(&[]),
    positions: &context.style.background_position,
    sizes: &context.style.background_size,
    repeats: &context.style.background_repeat,
    blend_modes: &context.style.background_blend_mode,
    context,
    area: origin.size.map(|x| x.max(0.0) as u32),
    paint: border_box.map(|x| x as u32),
    origin_offset: Point {
      x: origin.offset.x as i32,
      y: origin.offset.y as i32,
    },
  })
}

pub(crate) fn collect_background_layers(
  context: &RenderContext,
  layout: Layout,
) -> Result<TileLayers> {
  let border_box = layout.size;
  // `background-origin` sets the positioning area that `background-position`/`-size`
  // resolve against; `repeat` still tiles across the painting (border) box so a
  // repeating layer covers the clip region when origin and clip differ.
  let origin = background_origin_box(context.style.background_origin, layout);

  let mut layers = resolve_tile_layers(BackgroundLayersInput {
    images: context.style.background_image.as_deref().unwrap_or(&[]),
    positions: &context.style.background_position,
    sizes: &context.style.background_size,
    repeats: &context.style.background_repeat,
    blend_modes: &context.style.background_blend_mode,
    context,
    area: origin.size.map(|x| x.max(0.0) as u32),
    paint: border_box.map(|x| x as u32),
    origin_offset: Point {
      x: origin.offset.x as i32,
      y: origin.offset.y as i32,
    },
  })?;

  let background_color = context
    .style
    .background_color
    .resolve(context.current_color);

  if background_color.0[3] > 0 {
    layers.insert(
      0,
      TileLayer {
        tile: BackgroundTile::Color(ColorTile::new(
          background_color,
          border_box.width as u32,
          border_box.height as u32,
        )),
        xs: [0].into(),
        ys: [0].into(),
        blend_mode: BlendMode::Normal,
      },
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
      let mut rows = Vec::new();
      for y in [0, 1] {
        let mut row = vec![0u8; 64 * 4];
        tile.rasterize_row(y, 64, &mut row);
        rows.push(row);
      }
      rows
    };

    let plain = make_row(false);
    assert_eq!(plain[0], plain[1]);

    let dithered = make_row(true);
    assert_ne!(dithered[0], dithered[1]);
  }

  /// Opaque so the premultiplied round-trip through the canvas is lossless and
  /// the rendered bytes can be compared against the source directly.
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

  /// A background drawn at the source's own size is a copy of the source, so
  /// every `image-rendering` value has to agree with it and with each other.
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

  /// The fast path must not swallow `image-rendering` for a genuinely scaled
  /// background: nearest and the smooth samplers disagree there.
  #[test]
  fn scaled_background_still_honours_image_rendering() {
    let (_, source) = opaque_source(64, 48);

    let auto = render_background(source.clone(), (32, 24), ImageScalingAlgorithm::Auto);
    let pixelated = render_background(source, (32, 24), ImageScalingAlgorithm::Pixelated);

    assert_ne!(auto, pixelated);
  }
}
