use std::{borrow::Cow, iter::successors};

use image::{GenericImageView, Rgba, RgbaImage};
use smallvec::{SmallVec, smallvec};
use taffy::Size;

use crate::{
  Result,
  layout::{node::resolve_image, style::*},
  rendering::{BorderProperties, MaskMemory, RenderContext, Sizing, overlay_image},
};

pub(crate) struct TileLayer {
  pub tile: BackgroundTile,
  pub xs: SmallVec<[i32; 1]>,
  pub ys: SmallVec<[i32; 1]>,
}

pub(crate) type TileLayers = Vec<TileLayer>;

pub(crate) fn rasterize_layers(
  layers: TileLayers,
  size: Size<u32>,
  context: &RenderContext,
  border: BorderProperties,
  transform: Affine,
  mask_memory: &mut MaskMemory,
) -> Option<BackgroundTile> {
  if layers.is_empty() {
    return None;
  }

  let mut composed = RgbaImage::new(size.width, size.height);

  for layer in layers {
    for &x in &layer.xs {
      for &y in &layer.ys {
        overlay_image(
          &mut composed,
          &layer.tile,
          border,
          Affine::translation(x as f32, y as f32) * transform,
          context.style.image_rendering,
          255,
          None,
          mask_memory,
        );
      }
    }
  }

  Some(BackgroundTile::Image(composed))
}

pub(crate) enum BackgroundTile {
  Linear(LinearGradientTile),
  Radial(RadialGradientTile),
  Noise(NoiseV1Tile),
  Image(RgbaImage),
}

impl GenericImageView for BackgroundTile {
  type Pixel = Rgba<u8>;

  fn dimensions(&self) -> (u32, u32) {
    match self {
      Self::Linear(t) => t.dimensions(),
      Self::Radial(t) => t.dimensions(),
      Self::Noise(t) => t.dimensions(),
      Self::Image(t) => t.dimensions(),
    }
  }

  fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
    match self {
      Self::Linear(t) => t.get_pixel(x, y),
      Self::Radial(t) => t.get_pixel(x, y),
      Self::Noise(t) => t.get_pixel(x, y),
      Self::Image(t) => *t.get_pixel(x, y),
    }
  }
}

pub(crate) fn resolve_length_against_area(unit: Length, area: u32, sizing: &Sizing) -> u32 {
  match unit {
    Length::Auto => area,
    _ => unit.to_px(sizing, area as f32).max(0.0) as u32,
  }
}

pub(crate) fn resolve_background_size(
  size: BackgroundSize,
  area: (u32, u32),
  image: &BackgroundImage,
  context: &RenderContext,
) -> (u32, u32) {
  match size {
    BackgroundSize::Explicit { width, height } => (
      resolve_length_against_area(width, area.0, &context.sizing),
      resolve_length_against_area(height, area.1, &context.sizing),
    ),
    BackgroundSize::Cover => {
      // Get intrinsic image dimensions
      let (intrinsic_width, intrinsic_height) = if let BackgroundImage::Url(url) = image
        && let Ok(source) = resolve_image(url, context)
      {
        source.size()
      } else {
        return (0, 0);
      };

      if intrinsic_width == 0.0 || intrinsic_height == 0.0 {
        return (0, 0);
      }

      // Calculate scale factors for both dimensions
      let scale_x = area.0 as f32 / intrinsic_width;
      let scale_y = area.1 as f32 / intrinsic_height;

      // Use the larger scale to ensure the image covers the entire area
      let scale = scale_x.max(scale_y);

      (
        (intrinsic_width * scale).round() as u32,
        (intrinsic_height * scale).round() as u32,
      )
    }
    BackgroundSize::Contain => {
      // Get intrinsic image dimensions
      let (intrinsic_width, intrinsic_height) = if let BackgroundImage::Url(url) = image
        && let Ok(source) = resolve_image(url, context)
      {
        source.size()
      } else {
        return (0, 0);
      };

      if intrinsic_width == 0.0 || intrinsic_height == 0.0 {
        return (0, 0);
      }

      // Calculate scale factors for both dimensions
      let scale_x = area.0 as f32 / intrinsic_width;
      let scale_y = area.1 as f32 / intrinsic_height;

      // Use the smaller scale to ensure the image is fully contained
      let scale = scale_x.min(scale_y);

      (
        (intrinsic_width * scale).round() as u32,
        (intrinsic_height * scale).round() as u32,
      )
    }
  }
}

pub(crate) fn resolve_length_to_position_component(
  length: Length,
  available: i32,
  sizing: &Sizing,
) -> i32 {
  match length {
    Length::Auto => available / 2,
    _ => length.to_px(sizing, available as f32) as i32,
  }
}

pub(crate) fn resolve_position_component_x(
  comp: BackgroundPosition,
  tile_w: u32,
  area_w: u32,
  sizing: &Sizing,
) -> i32 {
  let available = area_w.saturating_sub(tile_w) as i32;
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
  sizing: &Sizing,
) -> i32 {
  let available = area_h.saturating_sub(tile_h) as i32;
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
      gradient, tile_w, tile_h, context,
    ))),
    BackgroundImage::Radial(gradient) => Some(BackgroundTile::Radial(RadialGradientTile::new(
      gradient, tile_w, tile_h, context,
    ))),
    BackgroundImage::Noise(noise) => Some(BackgroundTile::Noise(NoiseV1Tile::new(
      *noise, tile_w, tile_h,
    ))),
    BackgroundImage::Url(url) => {
      if let Ok(source) = resolve_image(url, context) {
        Some(BackgroundTile::Image(
          source
            .render_to_rgba_image(tile_w, tile_h, context.style.image_rendering)?
            .into_owned(),
        ))
      } else {
        None
      }
    }
  })
}

/// Resolve tile image, positions along X and Y for a background-like layer.
/// Returns (tile_image, tile_w, tile_h, xs, ys).
pub(crate) fn resolve_layer_tiles(
  image: &BackgroundImage,
  pos: BackgroundPosition,
  size: BackgroundSize,
  repeat: BackgroundRepeat,
  area_w: u32,
  area_h: u32,
  context: &RenderContext,
) -> Result<Option<TileLayer>> {
  let (initial_w, initial_h) = resolve_background_size(size, (area_w, area_h), image, context);

  if initial_w == 0 || initial_h == 0 {
    return Ok(None);
  }

  let (xs, tile_w) = match repeat.0 {
    BackgroundRepeatStyle::Repeat => {
      let origin_x = resolve_position_component_x(pos, initial_w, area_w, &context.sizing);
      (
        collect_repeat_tile_positions(area_w, initial_w, origin_x),
        initial_w,
      )
    }
    BackgroundRepeatStyle::NoRepeat => {
      let origin_x = resolve_position_component_x(pos, initial_w, area_w, &context.sizing);
      (smallvec![origin_x], initial_w)
    }
    BackgroundRepeatStyle::Space => (collect_spaced_tile_positions(area_w, initial_w), initial_w),
    BackgroundRepeatStyle::Round => collect_stretched_tile_positions(area_w, initial_w),
  };

  let (ys, tile_h) = match repeat.1 {
    BackgroundRepeatStyle::Repeat => {
      let origin_y = resolve_position_component_y(pos, initial_h, area_h, &context.sizing);
      (
        collect_repeat_tile_positions(area_h, initial_h, origin_y),
        initial_h,
      )
    }
    BackgroundRepeatStyle::NoRepeat => {
      let origin_y = resolve_position_component_y(pos, initial_h, area_h, &context.sizing);
      (smallvec![origin_y], initial_h)
    }
    BackgroundRepeatStyle::Space => (collect_spaced_tile_positions(area_h, initial_h), initial_h),
    BackgroundRepeatStyle::Round => collect_stretched_tile_positions(area_h, initial_h),
  };

  if xs.is_empty() || ys.is_empty() {
    return Ok(None);
  }

  let Some(tile) = render_tile(image, tile_w, tile_h, context)? else {
    return Ok(None);
  };

  Ok(Some(TileLayer { tile, xs, ys }))
}

/// Collects a list of tile positions to place along an axis.
/// Starts from the "origin" and collects tile positions until the "area_size" is reached.
pub(crate) fn collect_repeat_tile_positions(
  area_size: u32,
  tile_size: u32,
  origin: i32,
) -> SmallVec<[i32; 1]> {
  if tile_size == 0 {
    return SmallVec::default();
  }

  // Find first position, should be <= 0
  let mut start = origin;
  if start > 0 {
    let n = ((start as f32) / tile_size as f32).ceil() as i32;
    start -= n * tile_size as i32;
  }

  successors(Some(start), |&x| Some(x + tile_size as i32))
    .take_while(|&x| x < area_size as i32)
    .collect()
}

/// Collects evenly spaced tile positions along an axis for `background-repeat: space`.
/// Distributes gaps between tiles so the first and last touch the edges.
pub(crate) fn collect_spaced_tile_positions(area_size: u32, tile_size: u32) -> SmallVec<[i32; 1]> {
  if tile_size == 0 {
    return SmallVec::default();
  }

  // Calculate number of tiles that fit in the area
  let count = area_size / tile_size;

  // Fast path: if there's only one tile, center it
  if count <= 1 {
    return smallvec![(area_size as i32 - tile_size as i32) / 2];
  }

  // Calculate gap between tiles
  let gap = (area_size - count * tile_size) / (count - 1);
  let step = tile_size as i32 + gap as i32;

  successors(Some(0i32), move |&x| Some(x + step))
    .take(count as usize)
    .collect()
}

/// Collects stretched tile positions along an axis for `background-repeat: round`.
/// Rounds the size of the tile to fill the area.
/// Returns the positions and the new tile size.
pub(crate) fn collect_stretched_tile_positions(
  area_size: u32,
  tile_size: u32,
) -> (SmallVec<[i32; 1]>, u32) {
  if tile_size == 0 || area_size == 0 {
    return (SmallVec::default(), tile_size);
  }

  // Calculate number of tiles that fit in the area, at least 1
  let count = (area_size as f32 / tile_size as f32).max(1.0) as u32;

  let new_tile_size = (area_size as f32 / count as f32) as u32;

  let positions = successors(Some(0i32), move |&x| Some(x + new_tile_size as i32))
    .take(count as usize)
    .collect();

  (positions, new_tile_size)
}

pub(crate) fn resolve_tile_layers(
  images: &[BackgroundImage],
  positions: &[BackgroundPosition],
  sizes: &[BackgroundSize],
  repeats: &[BackgroundRepeat],
  context: &RenderContext,
  border_box: Size<u32>,
) -> Result<TileLayers> {
  let last_position = positions.last().copied().unwrap_or_default();
  let last_size = sizes.last().copied().unwrap_or_default();
  let last_repeat = repeats.last().copied().unwrap_or_default();

  let map_fn = |(i, image)| {
    let pos = positions.get(i).copied().unwrap_or(last_position);
    let size = sizes.get(i).copied().unwrap_or(last_size);
    let repeat = repeats.get(i).copied().unwrap_or(last_repeat);

    resolve_layer_tiles(
      image,
      pos,
      size,
      repeat,
      border_box.width,
      border_box.height,
      context,
    )
  };

  // Paint each background layer in order
  #[cfg(feature = "rayon")]
  {
    use rayon::prelude::*;

    let results = images
      .par_iter()
      .with_min_len(2)
      .enumerate()
      .map(map_fn)
      .collect::<Result<Vec<Option<TileLayer>>>>()?;

    Ok(results.into_iter().flatten().collect())
  }

  #[cfg(not(feature = "rayon"))]
  {
    let results = images
      .iter()
      .enumerate()
      .map(map_fn)
      .collect::<Result<Vec<Option<TileLayer>>>>()?;

    Ok(results.into_iter().flatten().collect())
  }
}

pub(crate) fn create_mask(
  context: &RenderContext,
  border_box: Size<f32>,
  mask_memory: &mut MaskMemory,
) -> Result<Option<Vec<u8>>> {
  let mask_image = context
    .style
    .mask_image
    .as_deref()
    .map(Cow::Borrowed)
    .unwrap_or_else(|| {
      Cow::Owned(
        context
          .style
          .mask
          .iter()
          .map(|background| background.image.clone())
          .collect::<Vec<_>>(),
      )
    });

  let layers = resolve_tile_layers(
    &mask_image,
    &context
      .style
      .mask_position
      .as_deref()
      .map(Cow::Borrowed)
      .unwrap_or_else(|| {
        Cow::Owned(
          context
            .style
            .mask
            .iter()
            .map(|background| background.position)
            .collect::<Vec<_>>(),
        )
      }),
    &context
      .style
      .mask_size
      .as_deref()
      .map(Cow::Borrowed)
      .unwrap_or_else(|| {
        Cow::Owned(
          context
            .style
            .mask
            .iter()
            .map(|background| background.size)
            .collect::<Vec<_>>(),
        )
      }),
    &context
      .style
      .mask_repeat
      .as_deref()
      .map(Cow::Borrowed)
      .unwrap_or_else(|| {
        Cow::Owned(
          context
            .style
            .mask
            .iter()
            .map(|background| background.repeat)
            .collect::<Vec<_>>(),
        )
      }),
    context,
    border_box.map(|x| x as u32),
  )?;

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
      mask_memory,
    )
    .map(|tile| tile.pixels().map(|(_, _, pixel)| pixel.0[3]).collect()),
  )
}

pub(crate) fn collect_background_image_layers(
  context: &RenderContext,
  border_box: Size<f32>,
) -> Result<TileLayers> {
  let background_image = context
    .style
    .background_image
    .as_deref()
    .map(Cow::Borrowed)
    .unwrap_or_else(|| {
      Cow::Owned(
        context
          .style
          .background
          .iter()
          .map(|background| background.image.clone())
          .collect::<Vec<_>>(),
      )
    });

  resolve_tile_layers(
    &background_image,
    &context
      .style
      .background_position
      .as_deref()
      .map(Cow::Borrowed)
      .unwrap_or_else(|| {
        Cow::Owned(
          context
            .style
            .background
            .iter()
            .map(|background| background.position)
            .collect::<Vec<_>>(),
        )
      }),
    &context
      .style
      .background_size
      .as_deref()
      .map(Cow::Borrowed)
      .unwrap_or_else(|| {
        Cow::Owned(
          context
            .style
            .background
            .iter()
            .map(|background| background.size)
            .collect::<Vec<_>>(),
        )
      }),
    &context
      .style
      .background_repeat
      .as_deref()
      .map(Cow::Borrowed)
      .unwrap_or_else(|| {
        Cow::Owned(
          context
            .style
            .background
            .iter()
            .map(|background| background.repeat)
            .collect::<Vec<_>>(),
        )
      }),
    context,
    border_box.map(|x| x as u32),
  )
}
