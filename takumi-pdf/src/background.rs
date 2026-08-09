//! Placement of `background-image` layers: `background-size`, `-position` and
//! `-repeat` resolved to a tile size, a first-tile origin, and a tiling step.

use takumi_core::{
  context::RenderContext,
  geometry::Size,
  layout::background::auto_axis_from_intrinsic,
  style::{
    AutoBackgroundAxis, BackgroundRepeat, BackgroundRepeatStyle, BackgroundSize, IntrinsicSizing,
    Length, PositionComponent, PositionValue,
  },
};

/// The value for one layer: CSS cycles the shorter list over the layers.
pub(crate) fn cycled<T: Copy + Default>(values: &[T], index: usize) -> T {
  if values.is_empty() {
    return T::default();
  }
  values[index % values.len()]
}

/// Where one background layer's tiles land inside the positioning area.
pub(crate) struct Placement {
  /// Whether either axis tiles. A tiling axis becomes a pattern even when one
  /// tile would span the area, because the phase can still pull a second tile
  /// into view.
  pub(crate) tiles: bool,
  /// Tile size after `background-size`, and after `round` rescales it.
  pub(crate) tile: Size<f32>,
  /// Top-left of the first tile, relative to the positioning area.
  pub(crate) origin: (f32, f32),
  /// Distance between tile origins. Equals the tile size for `repeat`, grows
  /// for `space`, and covers the whole area on an axis that does not repeat.
  pub(crate) step: (f32, f32),
}

/// One axis of a tiled layer.
struct Axis {
  tile: f32,
  origin: f32,
  step: f32,
}

/// Resolves one layer's placement. An image layer carries its intrinsic
/// sizing, which `auto`, `cover` and `contain` resolve against; a gradient has
/// none, so those all resolve to the positioning area.
pub(crate) fn place(
  area: Size<f32>,
  size: BackgroundSize,
  position: PositionValue,
  repeat: BackgroundRepeat,
  intrinsic: Option<IntrinsicSizing>,
  context: &RenderContext,
) -> Placement {
  let (tile, auto) = tile_size(area, size, intrinsic, context);
  // `round` rescales the axis it applies to. An axis left `auto` follows from
  // the image's ratio, so it has to resolve after the one it depends on.
  let (x, y) = match auto {
    Some((AutoBackgroundAxis::Width, ratio)) => {
      let y = axis(area.height, tile.height, position.0.y, repeat.1, context);
      let width =
        auto_axis_from_intrinsic(AutoBackgroundAxis::Width, ratio, y.tile).unwrap_or(tile.width);

      (axis(area.width, width, position.0.x, repeat.0, context), y)
    }
    Some((AutoBackgroundAxis::Height, ratio)) => {
      let x = axis(area.width, tile.width, position.0.x, repeat.0, context);
      let height =
        auto_axis_from_intrinsic(AutoBackgroundAxis::Height, ratio, x.tile).unwrap_or(tile.height);

      (
        x,
        axis(area.height, height, position.0.y, repeat.1, context),
      )
    }
    None => (
      axis(area.width, tile.width, position.0.x, repeat.0, context),
      axis(area.height, tile.height, position.0.y, repeat.1, context),
    ),
  };

  Placement {
    tiles: repeat.0 != BackgroundRepeatStyle::NoRepeat
      || repeat.1 != BackgroundRepeatStyle::NoRepeat,
    tile: Size {
      width: x.tile,
      height: y.tile,
    },
    origin: (x.origin, y.origin),
    step: (x.step, y.step),
  }
}

/// The tile before repeat rescales it, and which axis the ratio still has to
/// settle once the other one is known.
type AutoAxis = Option<(AutoBackgroundAxis, Option<f32>)>;

fn tile_size(
  area: Size<f32>,
  size: BackgroundSize,
  intrinsic: Option<IntrinsicSizing>,
  context: &RenderContext,
) -> (Size<f32>, AutoAxis) {
  // An image resolves through the core §5.3 algorithm, in whole device
  // pixels like the raster backend. Gradients stay on the exact float path.
  if let Some(intrinsic) = intrinsic {
    let resolved = size.resolve(
      Size {
        width: area.width.max(0.0) as u32,
        height: area.height.max(0.0) as u32,
      },
      &context.sizing,
      intrinsic,
    );

    return (
      Size {
        width: resolved.width as f32,
        height: resolved.height as f32,
      },
      resolved
        .auto_axis
        .map(|axis| (axis, resolved.intrinsic_ratio)),
    );
  }
  let BackgroundSize::Explicit { width, height } = size else {
    return (area, None);
  };
  let resolve = |length: Length, available: f32| match length {
    Length::Auto => available,
    length => length.to_px(&context.sizing, available).max(0.0),
  };

  (
    Size {
      width: resolve(width, area.width),
      height: resolve(height, area.height),
    },
    None,
  )
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
  let anchor = position.resolve(context, area - tile);
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
      // The position still applies, against the rescaled tile.
      let anchor = position.resolve(context, area - rounded);

      Axis {
        tile: rounded,
        origin: anchor - (anchor / rounded).ceil() * rounded,
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

#[cfg(test)]
mod tests {
  use takumi_core::{
    Fonts,
    context::RenderContext,
    geometry::Size,
    style::{
      BackgroundRepeats, BackgroundSizes, FromCssStr, IntrinsicSizing, PositionValues,
      SizingContext,
    },
    viewport::Viewport,
  };

  use super::place;

  /// `round` rescales the axis it applies to, and an `auto` axis follows from
  /// the image's ratio rather than keeping the size it was asked for.
  #[test]
  fn an_auto_axis_follows_the_rounded_one() {
    let fonts = Fonts::default();
    let context = RenderContext::builder()
      .fonts(fonts.snapshot_with_fallbacks(None))
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::new((1200, 630)))
          .build(),
      )
      .build();
    let placement = place(
      Size {
        width: 1200.0,
        height: 630.0,
      },
      BackgroundSizes::from_css_str("auto 80px").unwrap()[0],
      PositionValues::from_css_str("left top").unwrap()[0],
      BackgroundRepeats::from_css_str("no-repeat round").unwrap()[0],
      Some(IntrinsicSizing {
        width: Some(512.0),
        height: Some(512.0),
        ratio: Some(1.0),
      }),
      &context,
    );

    // 630 fits eight 80px tiles once rounded, so each is 78.75 tall. The width
    // is `auto` against a square image, so it follows rather than staying 80.
    assert_eq!(placement.tile.height, 78.75);
    assert_eq!(placement.tile.width, 78.75);
  }
}
