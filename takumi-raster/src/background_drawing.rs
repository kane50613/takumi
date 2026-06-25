use std::sync::Arc;

use image::Rgba;
use smallvec::{SmallVec, smallvec};
use taffy::{Layout, Point, Size};
use tiny_skia::{IntSize, Pixmap, PixmapMut, PremultipliedColorU8};

#[cfg(feature = "svg")]
use crate::resources::image::RenderedImage;
use crate::{
  BorderProperties, BufferPool, OverlayOptions, PaintSource, RenderContext, Result,
  SamplingFootprint, fast_div_255, interpolate_with_footprint,
  layout::{node::resolve_image, style::*},
  overlay_gradient_tile, overlay_image, overlay_linear_gradient_tile, overlay_radial_gradient_tile,
  pixmap_from_buffer, pixmap_ref_from_buffer,
  resources::{image::ImageSource, image_buffer::ImageBuffer},
};

pub(crate) struct TileLayer {
  pub blend_mode: BlendMode,
  pub tile: BackgroundTile,
  pub xs: SmallVec<[i32; 1]>,
  pub ys: SmallVec<[i32; 1]>,
}

pub(crate) type TileLayers = Vec<TileLayer>;

#[derive(Clone, Copy)]
pub(crate) struct LayerTileStyle {
  pub pos: BackgroundPosition,
  pub size: BackgroundSize,
  pub repeat: BackgroundRepeat,
  pub blend_mode: BlendMode,
}

pub(crate) struct ResolveLayerTilesInput<'a> {
  pub area: Size<u32>,
  pub context: &'a RenderContext,
  pub buffer_pool: &'a mut BufferPool,
}

pub(crate) struct ResolveTileLayersInput<'a> {
  pub images: &'a [BackgroundImage],
  pub positions: &'a [BackgroundPosition],
  pub sizes: &'a [BackgroundSize],
  pub repeats: &'a [BackgroundRepeat],
  pub blend_modes: &'a [BlendMode],
  pub context: &'a RenderContext,
  pub border_box: Size<u32>,
  pub buffer_pool: &'a mut BufferPool,
}

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

fn rasterize_tile(tile: BackgroundTile, buffer_pool: &mut BufferPool) -> Result<BackgroundTile> {
  let (width, height) = tile.dimensions();
  let Some(size) = IntSize::from_wh(width, height) else {
    return Ok(tile);
  };
  let mut data = buffer_pool.acquire_dirty((width * height * 4) as usize);

  for y in 0..height {
    let row_offset = (y * width * 4) as usize;
    let dst_row = &mut data[row_offset..row_offset + (width * 4) as usize];
    tile.rasterize_row(y, width, dst_row);
  }

  let Some(pixmap) = Pixmap::from_vec(data, size) else {
    return Ok(tile);
  };
  Ok(BackgroundTile::Pixmap(Arc::new(pixmap)))
}

fn resolve_intrinsic_size(image: &BackgroundImage, context: &RenderContext) -> IntrinsicSizing {
  let BackgroundImage::Url(url) = image else {
    return IntrinsicSizing::default();
  };

  let Ok(source) = resolve_image(url, context) else {
    return IntrinsicSizing::default();
  };

  source.intrinsic_sizing(&context.sizing)
}

pub(crate) fn rasterize_layers(
  layers: TileLayers,
  size: Size<u32>,
  context: &RenderContext,
  border: BorderProperties,
  transform: Affine,
  buffer_pool: &mut BufferPool,
) -> Result<Option<BackgroundTile>> {
  if layers.is_empty() || size.width == 0 || size.height == 0 {
    return Ok(None);
  }

  let Some(pixmap_size) = IntSize::from_wh(size.width, size.height) else {
    return Ok(None);
  };
  let mut composed = buffer_pool.acquire((size.width * size.height * 4) as usize);
  let Some(mut pixmap) = PixmapMut::from_bytes(&mut composed, size.width, size.height) else {
    buffer_pool.release(composed);
    return Ok(None);
  };

  for layer in layers {
    for &x in &layer.xs {
      for &y in &layer.ys {
        let layer_transform = Affine::translation(x as f32, y as f32) * transform;
        if border.is_zero()
          && layer_transform.only_translation()
          && layer.blend_mode == BlendMode::Normal
        {
          let translation = layer_transform.decompose_translation();
          match &layer.tile {
            BackgroundTile::Linear(linear_gradient) => {
              overlay_linear_gradient_tile(
                &mut pixmap,
                linear_gradient,
                translation,
                layer.blend_mode,
                None,
              );
              continue;
            }
            BackgroundTile::Radial(radial_gradient) => {
              overlay_radial_gradient_tile(
                &mut pixmap,
                radial_gradient,
                translation,
                layer.blend_mode,
                None,
              );
              continue;
            }
            BackgroundTile::Conic(conic_gradient) => {
              overlay_gradient_tile(
                &mut pixmap,
                conic_gradient,
                translation,
                layer.blend_mode,
                None,
              );
              continue;
            }
            _ => {}
          }
        }

        overlay_image(
          &mut pixmap,
          &layer.tile,
          OverlayOptions {
            border,
            transform: layer_transform,
            algorithm: context.style.image_rendering,
            mode: layer.blend_mode,
            combined_mask: None,
          },
          buffer_pool,
        );
      }
    }
  }

  let Some(pixmap) = Pixmap::from_vec(composed, pixmap_size) else {
    return Ok(None);
  };
  Ok(Some(BackgroundTile::Pixmap(Arc::new(pixmap))))
}

pub(crate) fn release_rasterized_background_tile(
  tile: BackgroundTile,
  buffer_pool: &mut BufferPool,
) {
  if let BackgroundTile::Pixmap(pixmap) = tile
    && let Ok(pixmap) = Arc::try_unwrap(pixmap)
  {
    buffer_pool.release(pixmap.take());
  }
}

pub(crate) struct ColorTile {
  color: Color,
  premultiplied: PremultipliedColorU8,
  pub width: u32,
  pub height: u32,
}

impl ColorTile {
  pub(crate) fn new(color: Rgba<u8>, width: u32, height: u32) -> Self {
    let alpha = color.0[3] as u32;
    let premultiplied = PremultipliedColorU8::from_rgba(
      fast_div_255(color.0[0] as u32 * alpha),
      fast_div_255(color.0[1] as u32 * alpha),
      fast_div_255(color.0[2] as u32 * alpha),
      color.0[3],
    )
    .unwrap_or(PremultipliedColorU8::TRANSPARENT);
    Self {
      color: color.0.into(),
      premultiplied,
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
      Self::SampledBitmap {
        source,
        width,
        height,
        algo,
      } => {
        let logical_width = (*width).max(1);
        let logical_height = (*height).max(1);
        let source_width = source.width().max(1);
        let source_height = source.height().max(1);

        let mapped_x = (x as f32 + 0.5) * source_width as f32 / logical_width as f32;
        let mapped_y = (y as f32 + 0.5) * source_height as f32 / logical_height as f32;
        let footprint = SamplingFootprint::new(
          source_width as f32 / logical_width as f32,
          source_height as f32 / logical_height as f32,
        );

        let Some(pixmap_ref) = pixmap_ref_from_buffer(source.as_ref()) else {
          return PremultipliedColorU8::TRANSPARENT;
        };
        let source = PaintSource::from(pixmap_ref);
        interpolate_with_footprint(source, *algo, mapped_x, mapped_y, footprint)
          .unwrap_or(PremultipliedColorU8::TRANSPARENT)
      }
      Self::Color(t) => t.get_pixel(x, y),
    }
  }

  pub(crate) fn rasterize_row(&self, y: u32, width: u32, dst: &mut [u8]) {
    debug_assert_eq!(dst.len(), (width * 4) as usize);
    let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(dst);
    match self {
      Self::Linear(t) => {
        let lut_len = t.lut_len();
        let mut row_state = t.begin_row(0, y, lut_len);
        for chunk in pixels.iter_mut() {
          let lut_idx = t.next_lut_index(&mut row_state);
          let p = t.sample_at(lut_idx);
          *chunk = [p.red(), p.green(), p.blue(), p.alpha()];
        }
      }
      Self::Radial(t) => {
        let lut_len = t.lut_len();
        let mut row_state = t.begin_row(0, y, lut_len);
        for chunk in pixels.iter_mut() {
          let lut_idx = t.next_lut_index(&mut row_state);
          let p = t.sample_at(lut_idx);
          *chunk = [p.red(), p.green(), p.blue(), p.alpha()];
        }
      }
      Self::Conic(t) => {
        let lut_len = t.lut_len();
        let mut row_state = t.begin_row(0, y, lut_len);
        for chunk in pixels.iter_mut() {
          let lut_idx = t.next_lut_index(&mut row_state);
          let p = t.sample_at(lut_idx);
          *chunk = [p.red(), p.green(), p.blue(), p.alpha()];
        }
      }
      Self::Pixmap(t) => {
        let ps = PaintSource::from(t.as_ref());
        for (x, chunk) in pixels.iter_mut().enumerate() {
          let p = ps.get_pixel(x as u32, y);
          *chunk = [p.red(), p.green(), p.blue(), p.alpha()];
        }
      }
      Self::SampledBitmap { .. } => {
        for (x, chunk) in pixels.iter_mut().enumerate() {
          let p = self.get_pixel(x as u32, y);
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

fn resolve_axis_tiles(
  repeat: BackgroundRepeatStyle,
  pos: BackgroundPosition,
  tile_size: u32,
  area_size: u32,
  sizing: &SizingContext,
  is_x: bool,
) -> (SmallVec<[i32; 1]>, u32) {
  match repeat {
    BackgroundRepeatStyle::Repeat => {
      let origin = if is_x {
        resolve_position_component_x(pos, tile_size, area_size, sizing)
      } else {
        resolve_position_component_y(pos, tile_size, area_size, sizing)
      };
      (
        collect_repeat_tile_positions(area_size, tile_size, origin),
        tile_size,
      )
    }
    BackgroundRepeatStyle::NoRepeat => {
      let origin = if is_x {
        resolve_position_component_x(pos, tile_size, area_size, sizing)
      } else {
        resolve_position_component_y(pos, tile_size, area_size, sizing)
      };
      (smallvec![origin], tile_size)
    }
    BackgroundRepeatStyle::Space => (
      collect_spaced_tile_positions(area_size, tile_size),
      tile_size,
    ),
    BackgroundRepeatStyle::Round => collect_stretched_tile_positions(area_size, tile_size),
  }
}

fn resolve_auto_axis_from_intrinsic(
  auto_axis: AutoBackgroundAxis,
  intrinsic_ratio: Option<f32>,
  fixed_size: u32,
) -> Option<u32> {
  let ratio = intrinsic_ratio?;
  if ratio == 0.0 {
    return Some(0);
  }

  let resolved = match auto_axis {
    AutoBackgroundAxis::Width => fixed_size as f32 * ratio,
    AutoBackgroundAxis::Height => fixed_size as f32 / ratio,
  };

  Some(resolved.round() as u32)
}

pub(crate) fn resolve_length_to_position_component(
  length: Length,
  available: i32,
  sizing: &SizingContext,
) -> i32 {
  match length {
    Length::Auto => available / 2,
    _ => length.to_px(sizing, available as f32) as i32,
  }
}

fn calculate_available_space(area_size: u32, tile_size: u32) -> i32 {
  i32::try_from(area_size)
    .unwrap_or(i32::MAX)
    .saturating_sub_unsigned(tile_size)
}

pub(crate) fn resolve_position_component_x(
  comp: BackgroundPosition,
  tile_w: u32,
  area_w: u32,
  sizing: &SizingContext,
) -> i32 {
  let available = calculate_available_space(area_w, tile_w);
  match comp.0.x {
    PositionComponent::KeywordX(PositionKeywordX::Left) => 0,
    PositionComponent::KeywordX(PositionKeywordX::Center) => available / 2,
    PositionComponent::KeywordX(PositionKeywordX::Right) => available,
    PositionComponent::KeywordY(_) => available / 2,
    PositionComponent::Length(length) => {
      resolve_length_to_position_component(length, available, sizing)
    }
  }
}

pub(crate) fn resolve_position_component_y(
  comp: BackgroundPosition,
  tile_h: u32,
  area_h: u32,
  sizing: &SizingContext,
) -> i32 {
  let available = calculate_available_space(area_h, tile_h);
  match comp.0.y {
    PositionComponent::KeywordY(PositionKeywordY::Top) => 0,
    PositionComponent::KeywordY(PositionKeywordY::Center) => available / 2,
    PositionComponent::KeywordY(PositionKeywordY::Bottom) => available,
    PositionComponent::KeywordX(_) => available / 2,
    PositionComponent::Length(length) => {
      resolve_length_to_position_component(length, available, sizing)
    }
  }
}

/// Rasterize a single background image into a tile of the given size.
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
    ))),
    BackgroundImage::Radial(gradient) => Some(BackgroundTile::Radial(RadialGradientTile::new(
      gradient,
      tile_w,
      tile_h,
      &context.sizing,
      context.current_color,
    ))),
    BackgroundImage::Conic(gradient) => Some(BackgroundTile::Conic(ConicGradientTile::new(
      gradient,
      tile_w,
      tile_h,
      &context.sizing,
      context.current_color,
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
          ImageSource::Gif(gif) => Some(BackgroundTile::SampledBitmap {
            source: gif.frame_at_time_arc(context.time_ms),
            width: tile_w,
            height: tile_h,
            algo: context.style.image_rendering,
          }),
          #[cfg(feature = "svg")]
          ImageSource::Svg(..) => match source.render_for_layout(
            tile_w,
            tile_h,
            context.style.image_rendering,
            context.time_ms,
          )? {
            RenderedImage::Rasterized(buffer) => {
              pixmap_from_buffer(&buffer).map(|pixmap| BackgroundTile::Pixmap(Arc::new(pixmap)))
            }
            RenderedImage::Borrowed { .. } => None,
          },
        }
      } else {
        None
      }
    }
  })
}

/// Resolve tile image, positions along X and Y for a background-like layer.
pub(crate) fn resolve_layer_tiles(
  image: &BackgroundImage,
  style: LayerTileStyle,
  input: ResolveLayerTilesInput<'_>,
) -> Result<Option<TileLayer>> {
  let resolved_size = style.size.resolve(
    input.area,
    &input.context.sizing,
    resolve_intrinsic_size(image, input.context),
  );

  if resolved_size.width == 0 || resolved_size.height == 0 {
    return Ok(None);
  }

  let (xs, ys, tile_w, tile_h) = match resolved_size.auto_axis {
    Some(AutoBackgroundAxis::Width) => {
      let (ys, tile_h) = resolve_axis_tiles(
        style.repeat.1,
        style.pos,
        resolved_size.height,
        input.area.height,
        &input.context.sizing,
        false,
      );
      let tile_w = if style.repeat.1 == BackgroundRepeatStyle::Round {
        resolve_auto_axis_from_intrinsic(
          AutoBackgroundAxis::Width,
          resolved_size.intrinsic_ratio,
          tile_h,
        )
        .unwrap_or(resolved_size.width)
      } else {
        resolved_size.width
      };
      let (xs, tile_w) = resolve_axis_tiles(
        style.repeat.0,
        style.pos,
        tile_w,
        input.area.width,
        &input.context.sizing,
        true,
      );
      (xs, ys, tile_w, tile_h)
    }
    Some(AutoBackgroundAxis::Height) => {
      let (xs, tile_w) = resolve_axis_tiles(
        style.repeat.0,
        style.pos,
        resolved_size.width,
        input.area.width,
        &input.context.sizing,
        true,
      );
      let tile_h = if style.repeat.0 == BackgroundRepeatStyle::Round {
        resolve_auto_axis_from_intrinsic(
          AutoBackgroundAxis::Height,
          resolved_size.intrinsic_ratio,
          tile_w,
        )
        .unwrap_or(resolved_size.height)
      } else {
        resolved_size.height
      };
      let (ys, tile_h) = resolve_axis_tiles(
        style.repeat.1,
        style.pos,
        tile_h,
        input.area.height,
        &input.context.sizing,
        false,
      );
      (xs, ys, tile_w, tile_h)
    }
    None => {
      let (xs, tile_w) = resolve_axis_tiles(
        style.repeat.0,
        style.pos,
        resolved_size.width,
        input.area.width,
        &input.context.sizing,
        true,
      );
      let (ys, tile_h) = resolve_axis_tiles(
        style.repeat.1,
        style.pos,
        resolved_size.height,
        input.area.height,
        &input.context.sizing,
        false,
      );
      (xs, ys, tile_w, tile_h)
    }
  };

  if xs.is_empty() || ys.is_empty() {
    return Ok(None);
  }

  let Some(tile) = render_tile(image, tile_w, tile_h, input.context)? else {
    return Ok(None);
  };
  let tile = if should_rasterize_repeated_tile(&tile, &xs, &ys) {
    rasterize_tile(tile, input.buffer_pool)?
  } else {
    tile
  };

  Ok(Some(TileLayer {
    tile,
    xs,
    ys,
    blend_mode: style.blend_mode,
  }))
}

pub(crate) fn resolve_tile_layers(input: ResolveTileLayersInput<'_>) -> Result<TileLayers> {
  let last_position = input.positions.last().copied().unwrap_or_default();
  let last_size = input.sizes.last().copied().unwrap_or_default();
  let last_repeat = input.repeats.last().copied().unwrap_or_default();
  let last_blend_mode = input.blend_modes.last().copied().unwrap_or_default();

  let mut results = Vec::new();
  for (i, image) in input.images.iter().enumerate().rev() {
    let style = LayerTileStyle {
      pos: input.positions.get(i).copied().unwrap_or(last_position),
      size: input.sizes.get(i).copied().unwrap_or(last_size),
      repeat: input.repeats.get(i).copied().unwrap_or(last_repeat),
      blend_mode: input.blend_modes.get(i).copied().unwrap_or(last_blend_mode),
    };

    results.push(resolve_layer_tiles(
      image,
      style,
      ResolveLayerTilesInput {
        area: input.border_box,
        context: input.context,
        buffer_pool: input.buffer_pool,
      },
    )?);
  }

  Ok(results.into_iter().flatten().collect())
}

pub(crate) fn create_mask(
  context: &RenderContext,
  border_box: Size<f32>,
  buffer_pool: &mut BufferPool,
) -> Result<Option<Vec<u8>>> {
  let mask_image = context.style.mask_image.as_deref().unwrap_or(&[]);
  let mask_position = context.style.mask_position.as_ref();
  let mask_size = context.style.mask_size.as_ref();
  let mask_repeat = context.style.mask_repeat.as_ref();

  let layers = resolve_tile_layers(ResolveTileLayersInput {
    images: mask_image,
    positions: mask_position,
    sizes: mask_size,
    repeats: mask_repeat,
    blend_modes: &[], // no blending mode for mask
    context,
    border_box: border_box.map(|x| x as u32),
    buffer_pool,
  })?;

  if layers.is_empty() {
    return Ok(None);
  }

  Ok(
    rasterize_layers(
      layers,
      border_box.map(|x| x as u32),
      context,
      BorderProperties::default(),
      Affine::IDENTITY,
      buffer_pool,
    )?
    .map(|tile| {
      let (w, h) = tile.dimensions();
      let mut alpha = buffer_pool.acquire_dirty((w * h) as usize);

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

      release_rasterized_background_tile(tile, buffer_pool);

      alpha
    }),
  )
}

struct OriginBox {
  offset: Point<f32>,
  size: Size<f32>,
}

fn background_origin_box(origin: BackgroundOrigin, layout: Layout) -> OriginBox {
  let border = layout.border;
  let padding = layout.padding;
  let inset = |left: f32, right: f32, top: f32, bottom: f32| OriginBox {
    offset: Point { x: left, y: top },
    size: Size {
      width: layout.size.width - left - right,
      height: layout.size.height - top - bottom,
    },
  };

  match origin {
    BackgroundOrigin::BorderBox => OriginBox {
      offset: Point { x: 0.0, y: 0.0 },
      size: layout.size,
    },
    BackgroundOrigin::PaddingBox => inset(border.left, border.right, border.top, border.bottom),
    BackgroundOrigin::ContentBox => inset(
      border.left + padding.left,
      border.right + padding.right,
      border.top + padding.top,
      border.bottom + padding.bottom,
    ),
    _ => OriginBox {
      offset: Point { x: 0.0, y: 0.0 },
      size: layout.size,
    },
  }
}

pub(crate) fn collect_background_layers(
  context: &RenderContext,
  layout: Layout,
  buffer_pool: &mut BufferPool,
) -> Result<TileLayers> {
  let border_box = layout.size;
  // `background-origin` sets the positioning area; tiles resolve against it and
  // are then shifted to its offset within the border box.
  // ponytail: repeats fill the origin box, not the clip box — a divergence only
  // when origin and clip differ on a repeating layer.
  let origin = background_origin_box(context.style.background_origin, layout);

  let mut layers = resolve_tile_layers(ResolveTileLayersInput {
    images: context.style.background_image.as_deref().unwrap_or(&[]),
    positions: &context.style.background_position,
    sizes: &context.style.background_size,
    repeats: &context.style.background_repeat,
    blend_modes: &context.style.background_blend_mode,
    context,
    border_box: origin.size.map(|x| x.max(0.0) as u32),
    buffer_pool,
  })?;

  let dx = origin.offset.x as i32;
  let dy = origin.offset.y as i32;
  if dx != 0 || dy != 0 {
    for layer in &mut layers {
      layer.xs.iter_mut().for_each(|x| *x += dx);
      layer.ys.iter_mut().for_each(|y| *y += dy);
    }
  }

  let background_color = context
    .style
    .background_color
    .resolve(context.current_color);

  if background_color.0[3] > 0 {
    layers.insert(
      0,
      TileLayer {
        tile: BackgroundTile::Color(ColorTile::new(
          background_color.into(),
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
  use std::rc::Rc;

  use taffy::Size;

  use super::{resolve_position_component_x, resolve_position_component_y};
  use crate::layout::{
    Viewport,
    style::{
      BackgroundPosition, CalcArena, Length, PositionComponent, PositionKeywordX, PositionKeywordY,
      SizingContext, SpacePair,
    },
  };

  fn test_sizing() -> SizingContext {
    let viewport = Viewport::new((100, 100));
    SizingContext {
      viewport,
      container_size: Size::NONE,
      font_size: viewport.font_size,
      root_font_size: None,
      line_height: 0.0,
      root_line_height: None,
      calc_arena: Rc::new(CalcArena::default()),
    }
  }

  #[test]
  fn oversized_background_keywords_resolve_to_negative_offsets() {
    let sizing = test_sizing();
    let position = BackgroundPosition(SpacePair::from_pair(
      PositionComponent::KeywordX(PositionKeywordX::Right),
      PositionComponent::KeywordY(PositionKeywordY::Bottom),
    ));

    assert_eq!(
      resolve_position_component_x(position, 150, 100, &sizing),
      -50
    );
    assert_eq!(
      resolve_position_component_y(position, 150, 100, &sizing),
      -50
    );
  }

  #[test]
  fn oversized_background_percentages_use_signed_available_space() {
    let sizing = test_sizing();
    let position = BackgroundPosition(SpacePair::from_pair(
      PositionComponent::Length(Length::Percentage(25.0)),
      PositionComponent::Length(Length::Percentage(75.0)),
    ));

    assert_eq!(
      resolve_position_component_x(position, 140, 100, &sizing),
      -10
    );
    assert_eq!(
      resolve_position_component_y(position, 140, 100, &sizing),
      -30
    );
  }
}
