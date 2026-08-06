//! Placement of `background-image` layers: `background-size`, `-position` and
//! `-repeat` resolved to a tile size, a first-tile origin, and a tiling step.

use takumi_core::{
  context::RenderContext,
  geometry::Size,
  style::{
    BackgroundRepeat, BackgroundRepeatStyle, BackgroundSize, Length, PositionComponent,
    PositionValue,
  },
};

use crate::paint::position_axis;

/// The value for one layer: CSS cycles the shorter list over the layers.
pub(crate) fn cycled<T: Copy + Default>(values: &[T], index: usize) -> T {
  if values.is_empty() {
    return T::default();
  }
  values[index % values.len()]
}

/// Where one background layer's tiles land inside the positioning area.
pub(crate) struct Placement {
  /// Tile size after `background-size`, and after `round` rescales it.
  pub(crate) tile: Size<f32>,
  /// Top-left of the first tile, relative to the positioning area.
  pub(crate) origin: (f32, f32),
  /// Distance between tile origins. Equals the tile size for `repeat`, grows
  /// for `space`, and covers the whole area on an axis that does not repeat.
  pub(crate) step: (f32, f32),
}

impl Placement {
  /// Whether the layer tiles at all, i.e. whether one draw is not enough.
  pub(crate) fn repeats(&self, area: Size<f32>) -> bool {
    self.step.0 < area.width || self.step.1 < area.height
  }
}

/// One axis of a tiled layer.
struct Axis {
  tile: f32,
  origin: f32,
  step: f32,
}

/// Resolves one layer's placement. Gradients have no intrinsic size, so `auto`,
/// `cover` and `contain` all resolve to the positioning area.
pub(crate) fn place(
  area: Size<f32>,
  size: BackgroundSize,
  position: PositionValue,
  repeat: BackgroundRepeat,
  context: &RenderContext,
) -> Placement {
  let tile = tile_size(area, size, context);
  let x = axis(area.width, tile.width, position.0.x, repeat.0, context);
  let y = axis(area.height, tile.height, position.0.y, repeat.1, context);

  Placement {
    tile: Size {
      width: x.tile,
      height: y.tile,
    },
    origin: (x.origin, y.origin),
    step: (x.step, y.step),
  }
}

fn tile_size(area: Size<f32>, size: BackgroundSize, context: &RenderContext) -> Size<f32> {
  let BackgroundSize::Explicit { width, height } = size else {
    return area;
  };
  let resolve = |length: Length, available: f32| match length {
    Length::Auto => available,
    length => length.to_px(&context.sizing, available).max(0.0),
  };

  Size {
    width: resolve(width, area.width),
    height: resolve(height, area.height),
  }
}

/// A repeating axis starts one step before the anchor so the tiles also cover
/// the area's leading edge. `round` rescales the tile to fit a whole number of
/// them, and `space` keeps the tile but spreads the leftover between tiles.
fn axis(
  area: f32,
  tile: f32,
  position: PositionComponent,
  repeat: BackgroundRepeatStyle,
  context: &RenderContext,
) -> Axis {
  if tile <= 0.0 {
    return Axis {
      tile,
      origin: 0.0,
      step: area.max(1.0),
    };
  }
  let anchor = position_axis(position, context, area - tile);
  let once = Axis {
    tile,
    origin: anchor,
    step: area.max(tile),
  };

  match repeat {
    BackgroundRepeatStyle::NoRepeat => once,
    BackgroundRepeatStyle::Repeat => Axis {
      tile,
      origin: anchor - (anchor / tile).ceil() * tile,
      step: tile,
    },
    BackgroundRepeatStyle::Round => {
      let count = (area / tile).round().max(1.0);
      let rounded = area / count;

      Axis {
        tile: rounded,
        origin: 0.0,
        step: rounded,
      }
    }
    BackgroundRepeatStyle::Space => {
      let count = (area / tile).floor();

      if count < 2.0 {
        return once;
      }
      Axis {
        tile,
        origin: 0.0,
        step: tile + (area - count * tile) / (count - 1.0),
      }
    }
  }
}
