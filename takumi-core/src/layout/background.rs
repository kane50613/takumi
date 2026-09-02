//! Resolving where a background layer's tiles land.

use smallvec::{SmallVec, smallvec};

use crate::{
  context::RenderContext,
  geometry::{ComputedLayout as Layout, Point, Size},
  layout::node::resolve_image,
  style::properties::background_repeat::{
    collect_repeat_tile_positions, collect_spaced_tile_positions, collect_stretched_tile_positions,
  },
  style::{
    AutoBackgroundAxis, BackgroundImage, BackgroundOrigin, BackgroundRepeat, BackgroundRepeatStyle,
    BackgroundSize, BlendMode, IntrinsicSizing, Length, PositionComponent, PositionKeywordX,
    PositionKeywordY, PositionValue, SizingContext,
  },
};

/// Where one background layer's tiles land, in border-box coordinates.
pub struct BackgroundLayerGeometry {
  /// Tile origins on the x axis.
  pub xs: SmallVec<[i32; 1]>,
  /// Tile origins on the y axis.
  pub ys: SmallVec<[i32; 1]>,
  /// Width of one tile.
  pub tile_width: u32,
  /// Height of one tile.
  pub tile_height: u32,
  /// The layer's `background-blend-mode`.
  pub blend_mode: BlendMode,
}

/// The `background-*` longhands that apply to one layer.
#[derive(Clone, Copy)]
pub struct LayerTileStyle {
  /// `background-position`.
  pub pos: PositionValue,
  /// `background-size`.
  pub size: BackgroundSize,
  /// `background-repeat`.
  pub repeat: BackgroundRepeat,
  /// `background-blend-mode`.
  pub blend_mode: BlendMode,
}

/// The areas one background layer resolves against.
pub struct BackgroundLayerInput<'a> {
  /// `background-origin` positioning area: position and `space`/`round` basis.
  pub area: Size<u32>,
  /// Painting area (border box) that `repeat` tiles across.
  pub paint: Size<u32>,
  /// Offset of the positioning area within the border box.
  pub origin_offset: Point<i32>,
  /// The node's render context.
  pub context: &'a RenderContext,
}

/// Every background layer of a box.
pub struct BackgroundLayersInput<'a> {
  /// `background-image` in CSS order, so the first entry is the topmost layer.
  pub images: &'a [BackgroundImage],
  /// `background-position`, one per layer.
  pub positions: &'a [PositionValue],
  /// `background-size`, one per layer.
  pub sizes: &'a [BackgroundSize],
  /// `background-repeat`, one per layer.
  pub repeats: &'a [BackgroundRepeat],
  /// `background-blend-mode`, one per layer.
  pub blend_modes: &'a [BlendMode],
  /// The node's render context.
  pub context: &'a RenderContext,
  /// `background-origin` positioning area.
  pub area: Size<u32>,
  /// Painting area (border box) that `repeat` tiles across.
  pub paint: Size<u32>,
  /// Offset of the positioning area within the border box.
  pub origin_offset: Point<i32>,
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

struct AxisArea {
  area: u32,
  paint: u32,
  offset: i32,
}

impl AxisArea {
  /// Resolves tile origins on this axis, in border-box coordinates.
  fn tiles(
    &self,
    repeat: BackgroundRepeatStyle,
    pos: PositionValue,
    tile_size: u32,
    sizing: &SizingContext,
    is_x: bool,
  ) -> (SmallVec<[i32; 1]>, u32) {
    let anchor = |tile: u32| {
      self.offset
        + if is_x {
          resolve_position_component_x(pos, tile, self.area, sizing)
        } else {
          resolve_position_component_y(pos, tile, self.area, sizing)
        }
    };
    let shift = |mut positions: SmallVec<[i32; 1]>| {
      if self.offset != 0 {
        positions.iter_mut().for_each(|x| *x += self.offset);
      }
      positions
    };

    match repeat {
      BackgroundRepeatStyle::Repeat => (
        collect_repeat_tile_positions(self.paint, tile_size, anchor(tile_size)),
        tile_size,
      ),
      BackgroundRepeatStyle::NoRepeat => (smallvec![anchor(tile_size)], tile_size),
      BackgroundRepeatStyle::Space => (
        shift(collect_spaced_tile_positions(self.area, tile_size)),
        tile_size,
      ),
      BackgroundRepeatStyle::Round => {
        let (positions, rounded) = collect_stretched_tile_positions(self.area, tile_size);
        (shift(positions), rounded)
      }
    }
  }
}

/// The size of a `background-size` axis left `auto`, taken from the image's ratio once the other
/// axis is settled.
pub fn auto_axis_from_intrinsic(
  auto_axis: AutoBackgroundAxis,
  intrinsic_ratio: Option<f32>,
  fixed_size: f32,
) -> Option<f32> {
  let ratio = intrinsic_ratio?;

  if ratio == 0.0 {
    return Some(0.0);
  }

  Some(match auto_axis {
    AutoBackgroundAxis::Width => fixed_size * ratio,
    AutoBackgroundAxis::Height => fixed_size / ratio,
  })
}

fn resolve_auto_axis_from_intrinsic(
  auto_axis: AutoBackgroundAxis,
  intrinsic_ratio: Option<f32>,
  fixed_size: u32,
) -> Option<u32> {
  auto_axis_from_intrinsic(auto_axis, intrinsic_ratio, fixed_size as f32)
    .map(|size| size.round() as u32)
}

/// Resolves a `background-position` length against the free space on an axis.
fn resolve_length_to_position_component(
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

/// Resolves the x component of a `background-position`.
fn resolve_position_component_x(
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

/// Resolves the y component of a `background-position`.
fn resolve_position_component_y(
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

/// A `background-origin` positioning area.
pub struct OriginBox {
  /// Offset of the positioning area inside the border box.
  pub offset: Point<f32>,
  /// The positioning area.
  pub size: Size<f32>,
}

/// The positioning area `background-origin` selects, and its offset inside the border box.
pub fn background_origin_box(origin: BackgroundOrigin, layout: Layout) -> OriginBox {
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
  }
}

impl BackgroundLayerInput<'_> {
  /// Resolves where one layer's tiles land.
  pub fn resolve(
    &self,
    image: &BackgroundImage,
    style: LayerTileStyle,
  ) -> Option<BackgroundLayerGeometry> {
    let resolved_size = style.size.resolve(
      self.area,
      &self.context.sizing,
      resolve_intrinsic_size(image, self.context),
    );

    if resolved_size.width == 0 || resolved_size.height == 0 {
      return None;
    }

    let axis_x = AxisArea {
      area: self.area.width,
      paint: self.paint.width,
      offset: self.origin_offset.x,
    };
    let axis_y = AxisArea {
      area: self.area.height,
      paint: self.paint.height,
      offset: self.origin_offset.y,
    };

    let (xs, ys, tile_w, tile_h) = match resolved_size.auto_axis {
      Some(AutoBackgroundAxis::Width) => {
        let (ys, tile_h) = axis_y.tiles(
          style.repeat.1,
          style.pos,
          resolved_size.height,
          &self.context.sizing,
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
        let (xs, tile_w) = axis_x.tiles(
          style.repeat.0,
          style.pos,
          tile_w,
          &self.context.sizing,
          true,
        );
        (xs, ys, tile_w, tile_h)
      }
      Some(AutoBackgroundAxis::Height) => {
        let (xs, tile_w) = axis_x.tiles(
          style.repeat.0,
          style.pos,
          resolved_size.width,
          &self.context.sizing,
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
        let (ys, tile_h) = axis_y.tiles(
          style.repeat.1,
          style.pos,
          tile_h,
          &self.context.sizing,
          false,
        );
        (xs, ys, tile_w, tile_h)
      }
      None => {
        let (xs, tile_w) = axis_x.tiles(
          style.repeat.0,
          style.pos,
          resolved_size.width,
          &self.context.sizing,
          true,
        );
        let (ys, tile_h) = axis_y.tiles(
          style.repeat.1,
          style.pos,
          resolved_size.height,
          &self.context.sizing,
          false,
        );
        (xs, ys, tile_w, tile_h)
      }
    };

    if xs.is_empty() || ys.is_empty() {
      return None;
    }

    Some(BackgroundLayerGeometry {
      xs,
      ys,
      tile_width: tile_w,
      tile_height: tile_h,
      blend_mode: style.blend_mode,
    })
  }
}

impl BackgroundLayersInput<'_> {
  /// Resolves every layer in bottom-to-top paint order.
  pub fn resolve(&self) -> Vec<(usize, BackgroundLayerGeometry)> {
    let last_position = self.positions.last().copied().unwrap_or_default();
    let last_size = self.sizes.last().copied().unwrap_or_default();
    let last_repeat = self.repeats.last().copied().unwrap_or_default();
    let last_blend_mode = self.blend_modes.last().copied().unwrap_or_default();
    let layer = BackgroundLayerInput {
      area: self.area,
      paint: self.paint,
      origin_offset: self.origin_offset,
      context: self.context,
    };

    let mut results = Vec::new();
    for (i, image) in self.images.iter().enumerate().rev() {
      let style = LayerTileStyle {
        pos: self.positions.get(i).copied().unwrap_or(last_position),
        size: self.sizes.get(i).copied().unwrap_or(last_size),
        repeat: self.repeats.get(i).copied().unwrap_or(last_repeat),
        blend_mode: self.blend_modes.get(i).copied().unwrap_or(last_blend_mode),
      };

      if let Some(geometry) = layer.resolve(image, style) {
        results.push((i, geometry));
      }
    }

    results
  }
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
