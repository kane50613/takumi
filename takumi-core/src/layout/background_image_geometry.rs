//! Where a `background-image` or `mask-image` layer's tiles land: `background-size`,
//! `-position`, and `-repeat` resolved in exact floats, after Blink's
//! `BackgroundImageGeometry` (`third_party/blink/renderer/core/paint/background_image_geometry.cc`).

use smallvec::{SmallVec, smallvec};

use crate::{
  context::RenderContext,
  geometry::{ComputedLayout as Layout, Point, Rect, Size},
  layout::node::resolve_image,
  style::{
    AutoBackgroundAxis, BackgroundImage, BackgroundOrigin, BackgroundRepeat, BackgroundRepeatStyle,
    BackgroundSize, BlendMode, ComputedStyle, IntrinsicSizing, Length, PositionComponent,
    PositionValue,
  },
};

/// The value for one layer: CSS cycles the shorter list over the layers.
fn cycled<T: Copy + Default>(values: &[T], index: usize) -> T {
  if values.is_empty() {
    return T::default();
  }
  values[index % values.len()]
}

/// The `-size`, `-position`, `-repeat`, and `-blend-mode` lists of `background-*` or `mask-*`,
/// Blink's `FillLayer` chain for one image list.
pub struct FillLayers<'s> {
  sizes: &'s [BackgroundSize],
  positions: &'s [PositionValue],
  repeats: &'s [BackgroundRepeat],
  blend_modes: &'s [BlendMode],
}

impl<'s> FillLayers<'s> {
  /// The `background-*` lists.
  pub fn background(style: &'s ComputedStyle) -> Self {
    Self {
      sizes: &style.background_size,
      positions: &style.background_position,
      repeats: &style.background_repeat,
      blend_modes: &style.background_blend_mode,
    }
  }

  /// The `mask-*` lists, which blend nothing.
  pub fn mask(style: &'s ComputedStyle) -> Self {
    Self {
      sizes: &style.mask_size,
      positions: &style.mask_position,
      repeats: &style.mask_repeat,
      blend_modes: &[],
    }
  }

  /// `background-blend-mode` of layer `index`.
  pub fn blend_mode(&self, index: usize) -> BlendMode {
    cycled(self.blend_modes, index)
  }

  /// Where layer `index`, drawing `image`, lands inside the positioning `area`.
  pub fn geometry(
    &self,
    index: usize,
    image: &BackgroundImage,
    area: Size<f32>,
    context: &RenderContext,
  ) -> BackgroundImageGeometry {
    BackgroundImageGeometry::resolve(
      area,
      cycled(self.sizes, index),
      cycled(self.positions, index),
      cycled(self.repeats, index),
      layer_intrinsic(image, context),
      context,
    )
  }
}

/// Intrinsic sizing of a `url()` layer, which `background-size` resolves against.
fn layer_intrinsic(image: &BackgroundImage, context: &RenderContext) -> Option<IntrinsicSizing> {
  let BackgroundImage::Url(url) = image else {
    return None;
  };
  let source = resolve_image(url, context).ok()?;

  Some(source.intrinsic_sizing().scale(&context.sizing))
}

/// A `background-origin` positioning area.
pub struct OriginBox {
  /// Offset of the positioning area inside the border box.
  pub offset: Point<f32>,
  /// The positioning area.
  pub size: Size<f32>,
}

impl OriginBox {
  /// The positioning area `origin` selects on `layout`.
  pub fn new(origin: BackgroundOrigin, layout: Layout) -> Self {
    let border = layout.border;
    let padding = layout.padding;
    let inset = |left: f32, right: f32, top: f32, bottom: f32| Self {
      offset: Point { x: left, y: top },
      size: Size {
        width: layout.size.width - left - right,
        height: layout.size.height - top - bottom,
      },
    };

    match origin {
      BackgroundOrigin::BorderBox => Self {
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
}

impl AutoBackgroundAxis {
  /// The size of this `auto` axis, taken from the image's ratio once the other axis is `fixed_size`.
  pub fn size_from_intrinsic(self, intrinsic_ratio: Option<f32>, fixed_size: f32) -> Option<f32> {
    let ratio = intrinsic_ratio?;

    if ratio == 0.0 {
      return Some(0.0);
    }

    Some(match self {
      Self::Width => fixed_size * ratio,
      Self::Height => fixed_size / ratio,
    })
  }
}

/// Where one background layer's tiles land, in exact floats relative to the positioning area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackgroundImageGeometry {
  /// The size one tile draws at, after `background-size` and `round`.
  pub tile_size: Size<f32>,
  /// The point of the tile pattern that lands on the positioning area's top-left. On an axis that
  /// does not repeat, the negated offset of its one tile.
  pub phase: Point<f32>,
  /// Extra room between tiles, which only `space` adds.
  pub repeat_spacing: Size<f32>,
  /// Whether the tiles repeat horizontally.
  pub repeat_x: bool,
  /// Whether the tiles repeat vertically.
  pub repeat_y: bool,
}

/// One axis of a tiled layer.
struct TileAxis {
  tile: f32,
  origin: f32,
  spacing: f32,
  repeats: bool,
}

impl BackgroundImageGeometry {
  /// Resolves one layer's placement. An image layer carries its intrinsic
  /// sizing, which `auto`, `cover` and `contain` resolve against; a gradient has
  /// none, so those all resolve to the positioning area.
  fn resolve(
    area: Size<f32>,
    size: BackgroundSize,
    position: PositionValue,
    repeat: BackgroundRepeat,
    intrinsic: Option<IntrinsicSizing>,
    context: &RenderContext,
  ) -> Self {
    let (tile, auto) = tile_size(area, size, intrinsic, context);
    // `round` rescales the axis it applies to. An axis left `auto` follows from
    // the image's ratio, so it has to resolve after the one it depends on.
    let (x, y) = match auto {
      Some((AutoBackgroundAxis::Width, ratio)) => {
        let y = TileAxis::resolve(area.height, tile.height, position.0.y, repeat.1, context);
        let width = AutoBackgroundAxis::Width
          .size_from_intrinsic(ratio, y.tile)
          .unwrap_or(tile.width);

        (
          TileAxis::resolve(area.width, width, position.0.x, repeat.0, context),
          y,
        )
      }
      Some((AutoBackgroundAxis::Height, ratio)) => {
        let x = TileAxis::resolve(area.width, tile.width, position.0.x, repeat.0, context);
        let height = AutoBackgroundAxis::Height
          .size_from_intrinsic(ratio, x.tile)
          .unwrap_or(tile.height);

        (
          x,
          TileAxis::resolve(area.height, height, position.0.y, repeat.1, context),
        )
      }
      None => (
        TileAxis::resolve(area.width, tile.width, position.0.x, repeat.0, context),
        TileAxis::resolve(area.height, tile.height, position.0.y, repeat.1, context),
      ),
    };

    Self {
      tile_size: Size {
        width: x.tile,
        height: y.tile,
      },
      phase: Point {
        x: -x.origin,
        y: -y.origin,
      },
      repeat_spacing: Size {
        width: x.spacing,
        height: y.spacing,
      },
      repeat_x: x.repeats,
      repeat_y: y.repeats,
    }
  }

  /// Whether the tiles repeat on either axis.
  pub fn repeats(&self) -> bool {
    self.repeat_x || self.repeat_y
  }

  /// The first tile's top-left, relative to the positioning area.
  pub fn first_tile(&self) -> Point<f32> {
    Point {
      x: -self.phase.x,
      y: -self.phase.y,
    }
  }

  /// Distance between the origins of neighboring tiles.
  pub fn step(&self) -> Size<f32> {
    Size {
      width: self.tile_size.width + self.repeat_spacing.width,
      height: self.tile_size.height + self.repeat_spacing.height,
    }
  }

  /// Every tile origin on each axis that meets `paint`, a rectangle relative to the positioning
  /// area. A repeating axis tiles all of it; an axis that does not repeat keeps its one tile.
  pub fn tile_origins(&self, paint: Rect<f32>) -> (SmallVec<[f32; 1]>, SmallVec<[f32; 1]>) {
    let first = self.first_tile();
    let step = self.step();

    (
      axis_origins(first.x, step.width, self.repeat_x, paint.left, paint.right),
      axis_origins(first.y, step.height, self.repeat_y, paint.top, paint.bottom),
    )
  }
}

/// A layer's tiles snapped to whole device pixels, in border-box coordinates.
pub struct SnappedTiles {
  /// Tile origins on the x axis.
  pub xs: SmallVec<[i32; 1]>,
  /// Tile origins on the y axis.
  pub ys: SmallVec<[i32; 1]>,
  /// Width of one tile.
  pub width: u32,
  /// Height of one tile.
  pub height: u32,
}

impl BackgroundImageGeometry {
  /// The tiles meeting a `paint` border box, with the positioning area at `offset` inside it.
  /// Approximate: tiles land on whole pixels, so a fractional tile drifts up to half a pixel
  /// from the exact geometry. A repeating axis rounds its tile up so neighbors leave no seam.
  pub fn snap(&self, paint: Size<f32>, offset: Point<f32>) -> Option<SnappedTiles> {
    // A tile under half a pixel paints nothing, the way it rounds.
    if self.tile_size.width.round() <= 0.0 || self.tile_size.height.round() <= 0.0 {
      return None;
    }
    let snap_size =
      |size: f32, repeats: bool| (if repeats { size.ceil() } else { size.round() }) as u32;
    let width = snap_size(self.tile_size.width, self.repeat_x);
    let height = snap_size(self.tile_size.height, self.repeat_y);
    let (xs, ys) = self.tile_origins(Rect {
      left: -offset.x,
      top: -offset.y,
      right: paint.width - offset.x,
      bottom: paint.height - offset.y,
    });
    let snap = |origins: SmallVec<[f32; 1]>, offset: f32| -> SmallVec<[i32; 1]> {
      origins
        .into_iter()
        .map(|origin| (origin + offset).round() as i32)
        .collect()
    };

    Some(SnappedTiles {
      xs: snap(xs, offset.x),
      ys: snap(ys, offset.y),
      width,
      height,
    })
  }
}

/// Tile origins on one axis that meet `start..end`.
fn axis_origins(first: f32, step: f32, repeats: bool, start: f32, end: f32) -> SmallVec<[f32; 1]> {
  if !repeats || step <= 0.0 {
    return smallvec![first];
  }
  let lead = first - ((first - start) / step).ceil() * step;
  let count = ((end - lead) / step).ceil().max(0.0) as usize;

  (0..count).map(|index| lead + index as f32 * step).collect()
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
impl TileAxis {
  fn resolve(
    area: f32,
    tile: f32,
    position: PositionComponent,
    repeat: BackgroundRepeatStyle,
    context: &RenderContext,
  ) -> Self {
    if tile <= 0.0 {
      return Self {
        tile,
        origin: 0.0,
        spacing: 0.0,
        repeats: false,
      };
    }
    let anchor = position.resolve(context, area - tile);
    let once = Self {
      tile,
      origin: anchor,
      spacing: 0.0,
      repeats: false,
    };

    match repeat {
      BackgroundRepeatStyle::NoRepeat => once,
      BackgroundRepeatStyle::Repeat => Self {
        tile,
        origin: anchor - (anchor / tile).ceil() * tile,
        spacing: 0.0,
        repeats: true,
      },
      BackgroundRepeatStyle::Round => {
        let count = (area / tile).round().max(1.0);
        let rounded = area / count;
        // The position still applies, against the rescaled tile.
        let anchor = position.resolve(context, area - rounded);

        Self {
          tile: rounded,
          origin: anchor - (anchor / rounded).ceil() * rounded,
          spacing: 0.0,
          repeats: true,
        }
      }
      BackgroundRepeatStyle::Space => {
        let count = (area / tile).floor();

        if count < 2.0 {
          return once;
        }
        Self {
          tile,
          origin: 0.0,
          spacing: (area - count * tile) / (count - 1.0),
          repeats: true,
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    Fonts,
    context::RenderContext,
    geometry::Size,
    style::{
      BackgroundRepeats, BackgroundSizes, FromCssStr, IntrinsicSizing, PositionValues,
      SizingContext,
    },
    viewport::Viewport,
  };

  use super::{BackgroundImageGeometry, axis_origins};

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
    let placement = BackgroundImageGeometry::resolve(
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
    assert_eq!(placement.tile_size.height, 78.75);
    assert_eq!(placement.tile_size.width, 78.75);
  }

  #[test]
  fn a_tile_larger_than_the_area_yields_one_origin() {
    assert_eq!(
      axis_origins(0.0, f32::MAX, true, 0.0, 100.0).as_slice(),
      [0.0]
    );
  }

  #[test]
  fn a_far_negative_first_tile_starts_at_the_area_edge() {
    assert_eq!(
      axis_origins(-1.0e9, 10.0, true, 0.0, 30.0).as_slice(),
      [0.0, 10.0, 20.0]
    );
    assert_eq!(
      axis_origins(-25.0, 10.0, true, 0.0, 30.0).as_slice(),
      [-5.0, 5.0, 15.0, 25.0]
    );
  }
}
