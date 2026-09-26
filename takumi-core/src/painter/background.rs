//! A box's background resolved for painting, after Blink's `BoxBackgroundPaintContext`
//! (`third_party/blink/renderer/core/paint/box_background_paint_context.cc`).

use crate::{
  context::RenderContext,
  geometry::{ComputedLayout, Point, Size},
  layout::{
    background_image_geometry::{BackgroundLayer, FillLayers, OriginBox},
    border::BorderProperties,
    decoration::ClipBox,
  },
  painter::FillShape,
  style::{BackgroundClip, Color},
};

/// The area a background paints into, from `background-clip`.
#[derive(Debug, Clone, Copy)]
pub enum BackgroundClipArea {
  /// The border box, its corners from the border.
  BorderBox(BorderProperties),
  /// The padding or content box.
  Inner(ClipBox),
  /// The border ring alone.
  BorderArea(BorderProperties),
  /// The box's glyphs.
  Text,
}

impl BackgroundClipArea {
  /// The area the box at `layout` clips its background to, its corners from `border`.
  pub fn new(context: &RenderContext, layout: ComputedLayout, border: BorderProperties) -> Self {
    match context.style.background_clip {
      BackgroundClip::BorderBox => Self::BorderBox(border),
      BackgroundClip::PaddingBox => Self::Inner(ClipBox::padding_box(border, layout)),
      BackgroundClip::ContentBox => Self::Inner(ClipBox::content_box(border, layout)),
      BackgroundClip::BorderArea => Self::BorderArea(border),
      BackgroundClip::Text => Self::Text,
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
  /// Where the background paints.
  pub clip: BackgroundClipArea,
  /// The positioning area `background-origin` selects.
  pub origin: OriginBox,
  /// The border-box size.
  pub size: Size<f32>,
  /// `background-image` layers, bottom first.
  pub layers: Vec<BackgroundLayer<'c>>,
}

impl<'c> BoxBackground<'c> {
  /// Resolves the background of the box at `layout`, its corners from `border`.
  pub fn new(context: &'c RenderContext, layout: ComputedLayout, border: BorderProperties) -> Self {
    let style = &context.style;
    let color = style.background_color.resolve(context.current_color);
    let origin = OriginBox::new(style.background_origin, layout);

    Self {
      color: (color.0[3] != 0).then_some(color),
      clip: BackgroundClipArea::new(context, layout, border),
      origin,
      size: layout.size,
      layers: FillLayers::background(style).resolve(
        style.background_image.as_deref().unwrap_or_default(),
        origin.size,
        context,
      ),
    }
  }
}
