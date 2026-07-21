use std::sync::Arc;

use image::Rgba;
use smallvec::{SmallVec, smallvec};
use takumi_core::{
  geometry::{ComputedLayout as Layout, Point, Size},
  paint::{
    ConicGradientTile, GradientOverlayTile, LinearGradientTile, RadialGradientTile,
    collect_repeat_tile_positions, collect_spaced_tile_positions, collect_stretched_tile_positions,
  },
};
use tiny_skia::{IntSize, Pixmap, PixmapMut, PremultipliedColorU8};

#[cfg(feature = "svg")]
use crate::resources::image::RenderedImage;
use crate::{
  BorderProperties, DrawTarget, OverlayOptions, PaintSource, RenderContext, Result,
  SamplingFootprint, color_to_premultiplied, interpolate_with_footprint,
  layout::node::resolve_image,
  overlay_gradient_tile, overlay_image, overlay_linear_gradient_tile, overlay_radial_gradient_tile,
  pixmap_from_buffer, pixmap_ref_from_buffer,
  resources::{image::ImageSource, image_buffer::ImageBuffer},
  style::*,
  uninit_buffer,
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
  pub pos: PositionValue,
  pub size: BackgroundSize,
  pub repeat: BackgroundRepeat,
  pub blend_mode: BlendMode,
}

pub(crate) struct ResolveLayerTilesInput<'a> {
  /// `background-origin` positioning area: position and `space`/`round` basis.
  pub area: Size<u32>,
  /// Painting area (border box) that `repeat` tiles across.
  pub paint: Size<u32>,
  /// Offset of the positioning area within the border box.
  pub origin_offset: Point<i32>,
  pub context: &'a RenderContext,
}

pub(crate) struct ResolveTileLayersInput<'a> {
  pub images: &'a [BackgroundImage],
  pub positions: &'a [PositionValue],
  pub sizes: &'a [BackgroundSize],
  pub repeats: &'a [BackgroundRepeat],
  pub blend_modes: &'a [BlendMode],
  pub context: &'a RenderContext,
  /// `background-origin` positioning area.
  pub area: Size<u32>,
  /// Painting area (border box) that `repeat` tiles across.
  pub paint: Size<u32>,
  /// Offset of the positioning area within the border box.
  pub origin_offset: Point<i32>,
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

fn rasterize_tile(tile: BackgroundTile) -> Result<BackgroundTile> {
  let (width, height) = tile.dimensions();
  let Some(size) = IntSize::from_wh(width, height) else {
    return Ok(tile);
  };
  let mut data = uninit_buffer((width * height * 4) as usize);

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

  source.intrinsic_sizing().scale(&context.sizing)
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
  let mut composed = vec![0; (size.width * size.height * 4) as usize];
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
        {
          let translation = Point {
            x: layer_transform.x,
            y: layer_transform.y,
          };
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

    fn rasterize_gradient_row<T: GradientOverlayTile>(t: &T, y: u32, pixels: &mut [[u8; 4]]) {
      let lut_len = t.lut_len();
      let mut row_state = t.begin_row(0, y, lut_len);
      for chunk in pixels.iter_mut() {
        let lut_idx = t.next_lut_index(&mut row_state);
        let p = t.sample_at(lut_idx);
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

/// One axis of the `background-origin` positioning area within the border box.
/// `position` and `space`/`round` distribution resolve against `area`; `repeat`
/// tiles across `paint` (the painting/border box) so a repeating layer still
/// covers the clip region when origin and clip differ.
#[derive(Clone, Copy)]
struct AxisArea {
  area: u32,
  paint: u32,
  offset: i32,
}

/// Resolves tile origins on one axis, in border-box coordinates.
fn resolve_axis_tiles(
  repeat: BackgroundRepeatStyle,
  pos: PositionValue,
  tile_size: u32,
  axis: AxisArea,
  sizing: &SizingContext,
  is_x: bool,
) -> (SmallVec<[i32; 1]>, u32) {
  let anchor = |tile: u32| {
    axis.offset
      + if is_x {
        resolve_position_component_x(pos, tile, axis.area, sizing)
      } else {
        resolve_position_component_y(pos, tile, axis.area, sizing)
      }
  };
  let shift = |mut positions: SmallVec<[i32; 1]>| {
    if axis.offset != 0 {
      positions.iter_mut().for_each(|x| *x += axis.offset);
    }
    positions
  };

  match repeat {
    BackgroundRepeatStyle::Repeat => (
      collect_repeat_tile_positions(axis.paint, tile_size, anchor(tile_size)),
      tile_size,
    ),
    BackgroundRepeatStyle::NoRepeat => (smallvec![anchor(tile_size)], tile_size),
    BackgroundRepeatStyle::Space => (
      shift(collect_spaced_tile_positions(axis.area, tile_size)),
      tile_size,
    ),
    BackgroundRepeatStyle::Round => {
      let (positions, rounded) = collect_stretched_tile_positions(axis.area, tile_size);
      (shift(positions), rounded)
    }
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
  comp: PositionValue,
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
  comp: PositionValue,
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
            source: gif.frame_at_time_covering(
              context.time_ms,
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
            context.time_ms,
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
            context.time_ms,
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

  let axis_x = AxisArea {
    area: input.area.width,
    paint: input.paint.width,
    offset: input.origin_offset.x,
  };
  let axis_y = AxisArea {
    area: input.area.height,
    paint: input.paint.height,
    offset: input.origin_offset.y,
  };

  let (xs, ys, tile_w, tile_h) = match resolved_size.auto_axis {
    Some(AutoBackgroundAxis::Width) => {
      let (ys, tile_h) = resolve_axis_tiles(
        style.repeat.1,
        style.pos,
        resolved_size.height,
        axis_y,
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
        axis_x,
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
        axis_x,
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
        axis_y,
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
        axis_x,
        &input.context.sizing,
        true,
      );
      let (ys, tile_h) = resolve_axis_tiles(
        style.repeat.1,
        style.pos,
        resolved_size.height,
        axis_y,
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
    rasterize_tile(tile)?
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
        area: input.area,
        paint: input.paint,
        origin_offset: input.origin_offset,
        context: input.context,
      },
    )?);
  }

  Ok(results.into_iter().flatten().collect())
}

pub(crate) fn create_mask(
  context: &RenderContext,
  border_box: Size<f32>,
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
    area: border_box.map(|x| x as u32),
    paint: border_box.map(|x| x as u32),
    origin_offset: Point { x: 0, y: 0 },
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
    )?
    .map(|tile| {
      let (w, h) = tile.dimensions();
      let mut alpha = uninit_buffer((w * h) as usize);

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
) -> Result<TileLayers> {
  let border_box = layout.size;
  // `background-origin` sets the positioning area that `background-position`/`-size`
  // resolve against; `repeat` still tiles across the painting (border) box so a
  // repeating layer covers the clip region when origin and clip differ.
  let origin = background_origin_box(context.style.background_origin, layout);

  let mut layers = resolve_tile_layers(ResolveTileLayersInput {
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
  use super::{resolve_position_component_x, resolve_position_component_y};
  use crate::{
    style::{
      Length, PositionComponent, PositionKeywordX, PositionKeywordY, PositionValue, SizingContext,
      SpacePair,
    },
    viewport::Viewport,
  };

  fn test_sizing() -> SizingContext {
    let viewport = Viewport::new((100, 100));
    SizingContext::builder()
      .viewport(viewport)
      .font_size(viewport.font_size)
      .line_height(0.0)
      .build()
  }

  #[test]
  fn oversized_background_keywords_resolve_to_negative_offsets() {
    let sizing = test_sizing();
    let position = PositionValue(SpacePair::from_pair(
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
    let position = PositionValue(SpacePair::from_pair(
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
