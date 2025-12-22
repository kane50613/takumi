use std::{borrow::Cow, iter::successors};

use image::RgbaImage;
use smallvec::{SmallVec, smallvec};
use taffy::{Point, Size};

use crate::{
  Result,
  layout::{node::resolve_image, style::*},
  rendering::{
    BorderProperties, Canvas, MaskMemory, RenderContext, Sizing, fast_resize, overlay_image,
  },
};

pub(crate) type ImageTiles = (RgbaImage, SmallVec<[i32; 1]>, SmallVec<[i32; 1]>);

pub(crate) fn resolve_length_against_area(unit: LengthUnit, area: u32, sizing: &Sizing) -> u32 {
  match unit {
    LengthUnit::Auto => area,
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

pub(crate) fn resolve_length_unit_to_position_component(
  length: LengthUnit,
  available: i32,
  sizing: &Sizing,
) -> i32 {
  match length {
    LengthUnit::Auto => available / 2,
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
      resolve_length_unit_to_position_component(length, available, sizing)
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
      resolve_length_unit_to_position_component(length, available, sizing)
    }
  }
}

/// Rasterize a single background image into a tile of the given size.
pub(crate) fn render_tile(
  image: &BackgroundImage,
  tile_w: u32,
  tile_h: u32,
  context: &RenderContext,
) -> Result<Option<RgbaImage>> {
  Ok(match image {
    BackgroundImage::None => None,
    BackgroundImage::Linear(gradient) => Some(gradient.to_image(tile_w, tile_h, context)),
    BackgroundImage::Radial(gradient) => Some(gradient.to_image(tile_w, tile_h, context)),
    BackgroundImage::Noise(noise) => Some(noise.to_image(tile_w, tile_h, context)),
    BackgroundImage::Url(url) => {
      if let Ok(source) = resolve_image(url, context) {
        Some(
          source
            .render_to_rgba_image(tile_w, tile_h, context.style.image_rendering)?
            .into_owned(),
        )
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
) -> Result<Option<ImageTiles>> {
  // Compute tile size
  let (mut tile_w, mut tile_h) = resolve_background_size(size, (area_w, area_h), image, context);

  if tile_w == 0 || tile_h == 0 {
    return Ok(None);
  }

  let Some(mut tile_image) = render_tile(image, tile_w, tile_h, context)? else {
    return Ok(None);
  };

  let xs: SmallVec<[i32; 1]> = match repeat.0 {
    BackgroundRepeatStyle::Repeat => {
      let origin_x = resolve_position_component_x(pos, tile_w, area_w, &context.sizing);
      collect_repeat_tile_positions(area_w, tile_w, origin_x)
    }
    BackgroundRepeatStyle::NoRepeat => {
      let origin_x = resolve_position_component_x(pos, tile_w, area_w, &context.sizing);
      smallvec![origin_x]
    }
    BackgroundRepeatStyle::Space => collect_spaced_tile_positions(area_w, tile_w),
    BackgroundRepeatStyle::Round => {
      let (px, new_w) = collect_stretched_tile_positions(area_w, tile_w);
      if new_w != tile_w {
        tile_w = new_w;
        tile_image = fast_resize(&tile_image, tile_w, tile_h, context.style.image_rendering)?;
      }
      px
    }
  };

  let ys: SmallVec<[i32; 1]> = match repeat.1 {
    BackgroundRepeatStyle::Repeat => {
      let origin_y = resolve_position_component_y(pos, tile_h, area_h, &context.sizing);
      collect_repeat_tile_positions(area_h, tile_h, origin_y)
    }
    BackgroundRepeatStyle::NoRepeat => {
      let origin_y = resolve_position_component_y(pos, tile_h, area_h, &context.sizing);
      smallvec![origin_y]
    }
    BackgroundRepeatStyle::Space => collect_spaced_tile_positions(area_h, tile_h),
    BackgroundRepeatStyle::Round => {
      let (py, new_h) = collect_stretched_tile_positions(area_h, tile_h);
      if new_h != tile_h {
        tile_h = new_h;
        tile_image = fast_resize(&tile_image, tile_w, tile_h, context.style.image_rendering)?;
      }
      py
    }
  };

  Ok(Some((tile_image, xs, ys)))
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

pub(crate) fn resolve_layers_tiles(
  images: &[BackgroundImage],
  positions: &[BackgroundPosition],
  sizes: &[BackgroundSize],
  repeats: &[BackgroundRepeat],
  context: &RenderContext,
  border_box: Size<f32>,
) -> Result<Vec<ImageTiles>> {
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
      border_box.width as u32,
      border_box.height as u32,
      context,
    )
  };

  // Paint each background layer in order
  #[cfg(feature = "rayon")]
  {
    use rayon::prelude::*;

    let results: Result<Vec<Option<ImageTiles>>> =
      images.par_iter().enumerate().map(map_fn).collect();
    Ok(results?.into_iter().flatten().collect())
  }

  #[cfg(not(feature = "rayon"))]
  {
    let results: Result<Vec<Option<ImageTiles>>> = images.iter().enumerate().map(map_fn).collect();
    Ok(results?.into_iter().flatten().collect())
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

  let resolved_tiles = resolve_layers_tiles(
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
    border_box,
  )?;

  if resolved_tiles.is_empty() {
    return Ok(None);
  }

  let mut composed = RgbaImage::new(border_box.width as u32, border_box.height as u32);

  for (tile_image, xs, ys) in resolved_tiles {
    for y in &ys {
      for x in &xs {
        overlay_image(
          &mut composed,
          (&tile_image).into(),
          Default::default(),
          Affine::translation(*x as f32, *y as f32),
          context.style.image_rendering,
          255,
          None,
          mask_memory,
        );
      }
    }
  }

  Ok(Some(composed.iter().skip(3).step_by(4).copied().collect()))
}

pub(crate) fn collect_background_image_tiles(
  context: &RenderContext,
  border_box: Size<f32>,
) -> Result<Vec<ImageTiles>> {
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

  resolve_layers_tiles(
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
    border_box,
  )
}

pub(crate) fn create_background_image(
  context: &RenderContext,
  border_box: Size<f32>,
  size: Size<f32>,
  offset: Point<f32>,
  mask_memory: &mut MaskMemory,
) -> Result<Option<RgbaImage>> {
  let resolved_tiles = collect_background_image_tiles(context, border_box)?;

  if resolved_tiles.is_empty() {
    return Ok(None);
  }

  // Fast path: If there is exactly one tile and is the desired size and position, we can just return the tile image
  if offset == Point::zero()
    && resolved_tiles.len() == 1
    && resolved_tiles.first().is_some_and(|(tile_image, xs, ys)| {
      tile_image.width() == size.width as u32
        && tile_image.height() == size.height as u32
        && xs.len() == 1
        && ys.len() == 1
        && xs.first().is_some_and(|x| *x == 0)
        && ys.first().is_some_and(|y| *y == 0)
    })
  {
    return Ok(
      resolved_tiles
        .into_iter()
        .next()
        .map(|(tile_image, _, _)| tile_image),
    );
  }

  let mut composed = RgbaImage::new(size.width as u32, size.height as u32);

  for (tile_image, xs, ys) in resolved_tiles {
    for y in &ys {
      for x in &xs {
        overlay_image(
          &mut composed,
          (&tile_image).into(),
          Default::default(),
          Affine::translation(*x as f32 - offset.x, *y as f32 - offset.y),
          context.style.image_rendering,
          255,
          None,
          mask_memory,
        );
      }
    }
  }

  Ok(Some(composed))
}

/// Draw layered backgrounds (gradients) with support for background-size, -position, and -repeat.
pub(crate) fn draw_background_layers(
  tiles: Vec<ImageTiles>,
  radius: BorderProperties,
  context: &RenderContext,
  canvas: &mut Canvas,
) {
  for (tile_image, xs, ys) in tiles {
    for y in &ys {
      for x in &xs {
        canvas.overlay_image(
          (&tile_image).into(),
          radius,
          context.transform * Affine::translation(*x as f32, *y as f32),
          ImageScalingAlgorithm::Auto,
          context.opacity,
        );
      }
    }
  }
}
