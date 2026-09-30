//! A box's background resolved for painting, after Blink's `BoxBackgroundPaintContext`
//! (`third_party/blink/renderer/core/paint/box_background_paint_context.cc`).

use crate::{
  context::RenderContext,
  geometry::{ComputedLayout, Point, Rect, Size},
  layout::{
    background_image_geometry::{BackgroundLayer, BoxBackgroundPaintContext, FillLayers},
    border::BorderProperties,
    decoration::{ClipBox, ContourOrigin},
  },
  painter::{FillShape, SnappedBox},
  style::{BackgroundClip, BorderStyle, Color, Sides},
};

/// The area a background paints into, from `background-clip`.
#[derive(Debug, Clone, Copy)]
pub enum BackgroundClipArea {
  /// The border box, its corners from the border.
  BorderBox(BorderProperties),
  /// The padding or content box.
  Inner(ClipBox),
  /// Where the border paints.
  BorderArea(BorderProperties),
  /// The box's glyphs.
  Text,
}

impl BackgroundClipArea {
  /// The area the box at `layout`, snapped to `snapped`, clips its background to, its corners
  /// from `border`, relative to the snapped box.
  pub fn new(
    context: &RenderContext,
    layout: ComputedLayout,
    border: BorderProperties,
    snapped: &SnappedBox,
  ) -> Self {
    let inner = |clip: ClipBox, insets: Rect<f32>| {
      let (offset, size) = if border.is_zero() {
        snapped.inset(insets)
      } else {
        snapped.contoured_inset(insets, true)
      };

      Self::Inner(ClipBox {
        offset,
        size,
        origin: clip.origin.map(|origin| ContourOrigin {
          size: snapped.size(),
          ..origin
        }),
        ..clip
      })
    };

    match context.style.background_clip {
      BackgroundClip::BorderBox => Self::BorderBox(border),
      BackgroundClip::PaddingBox => inner(ClipBox::padding_box(border, layout), layout.border),
      BackgroundClip::ContentBox => inner(
        ClipBox::content_box(border, layout),
        Rect {
          top: layout.border.top + layout.padding.top,
          right: layout.border.right + layout.padding.right,
          bottom: layout.border.bottom + layout.padding.bottom,
          left: layout.border.left + layout.padding.left,
        },
      ),
      BackgroundClip::BorderArea => Self::BorderArea(border),
      BackgroundClip::Text => Self::Text,
    }
  }

  /// The border a `border-area` background is masked by when a side leaves gaps, in opaque black,
  /// as Blink's `AllBordersFillBorderArea` sends such a border to a `DstIn` mask. A border without
  /// gaps clips to [`BackgroundClipArea::shape`]'s ring instead.
  pub fn border_mask(&self) -> Option<BorderProperties> {
    let Self::BorderArea(border) = self else {
      return None;
    };
    let gaps = border.sides().iter().any(|side| {
      side.width > 0.0
        && matches!(
          side.style,
          BorderStyle::Dotted | BorderStyle::Dashed | BorderStyle::Double
        )
    });

    gaps.then(|| BorderProperties {
      color: Sides([Color::black(); 4]).into(),
      ..*border
    })
  }

  /// The rectangle the clip region of a `size` border box lies in.
  pub fn bounds(&self, size: Size<f32>) -> Rect<f32> {
    let (offset, size) = match *self {
      Self::Inner(clip) => (clip.offset, clip.size),
      Self::BorderBox(_) | Self::BorderArea(_) | Self::Text => (Point::ZERO, size),
    };

    Rect {
      left: offset.x,
      top: offset.y,
      right: offset.x + size.width,
      bottom: offset.y + size.height,
    }
  }

  /// The clip region of a `size` border box, or `None` for `text` and an empty box.
  pub fn shape(&self, size: Size<f32>) -> Option<FillShape> {
    if size.width <= 0.0 || size.height <= 0.0 {
      return None;
    }

    match *self {
      Self::BorderBox(border) if border.is_zero() => Some(FillShape::Rect(size)),
      Self::BorderBox(border) => Some(FillShape::RoundedRect {
        border,
        size,
        offset: Point::ZERO,
      }),
      Self::Inner(clip) => Some(clip.into()),
      Self::BorderArea(border) => Some(FillShape::border_ring(&border, size)),
      Self::Text => None,
    }
  }
}

/// A box's background, resolved.
pub struct BoxBackground<'c> {
  /// `background-color`, when visible.
  pub color: Option<Color>,
  /// Where the background paints, relative to the snapped border box.
  pub clip: BackgroundClipArea,
  /// The snapped border box's top-left, relative to the border box.
  pub offset: Point<f32>,
  /// The snapped border box's size.
  pub size: Size<f32>,
  /// `background-image` layers, bottom first.
  pub layers: Vec<BackgroundLayer<'c>>,
}

impl<'c> BoxBackground<'c> {
  /// Resolves the background of the box at `layout`, its corners from `border`, its border box at
  /// `paint_offset` in the space its background snaps to pixels in.
  pub fn new(
    context: &'c RenderContext,
    layout: ComputedLayout,
    border: BorderProperties,
    paint_offset: Point<f32>,
  ) -> Self {
    let style = &context.style;
    let color = style.background_color.resolve(context.current_color);
    let snapped = SnappedBox::new(paint_offset, layout.size);

    Self {
      color: (color.0[3] != 0).then_some(color),
      clip: BackgroundClipArea::new(context, layout, border, &snapped),
      offset: snapped.offset(),
      size: snapped.size(),
      layers: FillLayers::background(style).resolve(
        style.background_image.as_deref().unwrap_or_default(),
        &BoxBackgroundPaintContext::new(style, layout, &border, paint_offset),
        context,
      ),
    }
  }
}
