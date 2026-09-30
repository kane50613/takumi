//! Where a `background-image` or `mask-image` layer paints: Blink's `BackgroundImageGeometry`
//! (`third_party/blink/renderer/core/paint/background_image_geometry.cc`) for a box painted in one
//! piece, the parts of `BoxBackgroundPaintContext` it reads, and the tiling `DrawTiledBackground`
//! (`box_painter_base.cc`) derives from it. Follows Blink under the notice in LICENSE-CHROMIUM.

use smallvec::SmallVec;

use crate::{
  context::RenderContext,
  geometry::{ComputedLayout, Point, Rect, Size},
  layout::{border::BorderProperties, node::resolve_image},
  layout_unit::{
    BoxStrut, LayoutUnit, UnitOffset, UnitRect, UnitSize, snap_size_to_pixel_allowing_zero,
  },
  resources::image::ImageSource,
  style::{
    BackgroundClip, BackgroundImage, BackgroundOrigin, BackgroundRepeat, BackgroundRepeatStyle,
    BackgroundSize, BlendMode, BorderCollapse, BorderStyle, ComputedStyle, IntrinsicSizing, Length,
    PositionComponent, PositionValue, SizingContext,
  },
};

/// The value for one layer: CSS cycles the shorter list over the layers.
fn cycled<T: Copy + Default>(values: &[T], index: usize) -> T {
  if values.is_empty() {
    return T::default();
  }
  values[index % values.len()]
}

/// One `background-image` or `mask-image` layer, resolved.
pub struct BackgroundLayer<'i> {
  /// The image the layer draws.
  pub image: &'i BackgroundImage,
  /// Where its tiles land.
  pub tiling: ImageTiling,
  /// Its `background-blend-mode`.
  pub blend_mode: BlendMode,
}

/// Blink's `EFillLayerType`: whether a layer paints a background or a mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FillLayerType {
  Background,
  Mask,
}

/// Blink's `EFillBox`: the box `background-clip` or `background-origin` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FillBox {
  Border,
  Padding,
  Content,
  Text,
  BorderArea,
}

impl From<BackgroundClip> for FillBox {
  fn from(clip: BackgroundClip) -> Self {
    match clip {
      BackgroundClip::BorderBox => Self::Border,
      BackgroundClip::PaddingBox => Self::Padding,
      BackgroundClip::ContentBox => Self::Content,
      BackgroundClip::Text => Self::Text,
      BackgroundClip::BorderArea => Self::BorderArea,
    }
  }
}

impl From<BackgroundOrigin> for FillBox {
  fn from(origin: BackgroundOrigin) -> Self {
    match origin {
      BackgroundOrigin::BorderBox => Self::Border,
      BackgroundOrigin::PaddingBox => Self::Padding,
      BackgroundOrigin::ContentBox => Self::Content,
    }
  }
}

/// The `-size`, `-position`, `-repeat`, `-clip`, `-origin`, and `-blend-mode` values of
/// `background-*` or `mask-*`, Blink's `FillLayer` chain for one image list.
pub struct FillLayers<'s> {
  kind: FillLayerType,
  sizes: &'s [BackgroundSize],
  positions: &'s [PositionValue],
  repeats: &'s [BackgroundRepeat],
  blend_modes: &'s [BlendMode],
  clip: FillBox,
  origin: FillBox,
}

impl<'s> FillLayers<'s> {
  /// The `background-*` lists.
  pub fn background(style: &'s ComputedStyle) -> Self {
    Self {
      kind: FillLayerType::Background,
      sizes: &style.background_size,
      positions: &style.background_position,
      repeats: &style.background_repeat,
      blend_modes: &style.background_blend_mode,
      clip: style.background_clip.into(),
      origin: style.background_origin.into(),
    }
  }

  /// The `mask-*` lists, which blend nothing and clip to and position against the border box.
  pub fn mask(style: &'s ComputedStyle) -> Self {
    Self {
      kind: FillLayerType::Mask,
      sizes: &style.mask_size,
      positions: &style.mask_position,
      repeats: &style.mask_repeat,
      blend_modes: &[],
      clip: FillBox::Border,
      origin: FillBox::Border,
    }
  }

  /// Every layer of `images` that paints over the box `paint_context` describes, bottom first.
  pub fn resolve<'i>(
    &self,
    images: &'i [BackgroundImage],
    paint_context: &BoxBackgroundPaintContext,
    context: &RenderContext,
  ) -> Vec<BackgroundLayer<'i>> {
    images
      .iter()
      .enumerate()
      .rev()
      .filter(|(_, image)| image.paints())
      .filter_map(|(index, image)| {
        let layer = self.layer(index, image, context);
        let geometry = BackgroundImageGeometry::calculate(
          &layer,
          paint_context,
          paint_context.border_box,
          &context.sizing,
        );

        Some(BackgroundLayer {
          image,
          tiling: geometry.tiling(&layer.image)?.shifted(Point {
            x: -paint_context.paint_offset.x,
            y: -paint_context.paint_offset.y,
          }),
          blend_mode: cycled(self.blend_modes, index),
        })
      })
      .collect()
  }

  fn layer(&self, index: usize, image: &BackgroundImage, context: &RenderContext) -> FillLayer {
    FillLayer {
      kind: self.kind,
      image: LayerImage::of(image, context),
      size: cycled(self.sizes, index),
      position: cycled(self.positions, index),
      repeat: cycled(self.repeats, index),
      clip: self.clip,
      origin: self.origin,
    }
  }
}

/// Blink's `FillLayer`: one layer's values.
struct FillLayer {
  kind: FillLayerType,
  image: LayerImage,
  size: BackgroundSize,
  position: PositionValue,
  repeat: BackgroundRepeat,
  clip: FillBox,
  origin: FillBox,
}

impl FillLayer {
  /// Blink's `PositionX` as a length.
  fn position_x(&self) -> Length {
    position_length(self.position.0.x)
  }

  /// Blink's `PositionY` as a length.
  fn position_y(&self) -> Length {
    position_length(self.position.0.y)
  }

  /// Blink's `SizeLength`, `auto` on both axes for `cover` and `contain`.
  fn size_length(&self) -> (Length, Length) {
    match self.size {
      BackgroundSize::Explicit { width, height } => (width, height),
      BackgroundSize::Cover | BackgroundSize::Contain => (Length::Auto, Length::Auto),
    }
  }
}

/// A position component as the length Blink stores, keywords as the percentages they compute to.
fn position_length(component: PositionComponent) -> Length {
  match Length::from(component) {
    Length::Auto => Length::Percentage(50.0),
    length => length,
  }
}

/// Blink's `StyleImage`, as the geometry and the tiling read it.
struct LayerImage {
  /// Blink's `NaturalSizingInfo`, its dimensions zoomed to device px.
  natural: IntrinsicSizing,
  /// Blink's `Image::SizeAsFloat` for a bitmap: the pixels it decodes to.
  bitmap_size: Option<Size<f32>>,
}

impl LayerImage {
  /// A gradient has no natural sizing; a `url()` has its source's.
  fn of(image: &BackgroundImage, context: &RenderContext) -> Self {
    let BackgroundImage::Url(url) = image else {
      return Self::generated();
    };
    let Ok(source) = resolve_image(url, context) else {
      return Self::generated();
    };
    let natural = source.intrinsic_sizing();
    let bitmap_size = match source {
      #[cfg(feature = "svg-sizing")]
      ImageSource::Svg(_) => None,
      _ => natural
        .width
        .zip(natural.height)
        .map(|(width, height)| Size { width, height }),
    };

    Self {
      natural: natural.scale(&context.sizing),
      bitmap_size,
    }
  }

  fn generated() -> Self {
    Self {
      natural: IntrinsicSizing::default(),
      bitmap_size: None,
    }
  }

  /// Blink's `HasIntrinsicSize`.
  fn has_intrinsic_size(&self) -> bool {
    !self.natural.is_none()
  }

  /// Blink's `ImageSize`: the concrete object size against `default_object_size`.
  fn image_size(&self, default_object_size: Size<f32>) -> Size<f32> {
    self.natural.concrete_object_size(default_object_size)
  }
}

/// Blink's `SnappedAndUnsnappedOutsets`.
#[derive(Debug, Clone, Copy, Default)]
struct SnappedAndUnsnappedOutsets {
  snapped: BoxStrut,
  unsnapped: BoxStrut,
}

/// Blink's `BoxBackgroundPaintContext` for a box painted in one piece.
#[derive(Debug, Clone, Copy)]
pub struct BoxBackgroundPaintContext {
  /// Where the border box sits in the space it snaps to pixels in.
  paint_offset: Point<f32>,
  border_box: UnitRect,
  border: BoxStrut,
  padding: BoxStrut,
  /// Blink's `BorderEdge::ObscuresBackground` per side.
  obscuring: Rect<bool>,
  disallow_border_derived_adjustment: bool,
}

impl BoxBackgroundPaintContext {
  /// The context of the box at `layout` with `style`, its borders from `border`, its border box at
  /// `paint_offset` in the space Blink snaps it in.
  pub fn new(
    style: &ComputedStyle,
    layout: ComputedLayout,
    border: &BorderProperties,
    paint_offset: Point<f32>,
  ) -> Self {
    let strut = |sides: Rect<f32>| BoxStrut {
      top: LayoutUnit::from_f32(sides.top),
      right: LayoutUnit::from_f32(sides.right),
      bottom: LayoutUnit::from_f32(sides.bottom),
      left: LayoutUnit::from_f32(sides.left),
    };
    let obscures = |width: f32, color: [u8; 4], style: BorderStyle| {
      obscures_background(width as i32, color[3] == u8::MAX, style)
    };

    Self {
      paint_offset,
      border_box: UnitRect::nearest(paint_offset, layout.size),
      border: strut(layout.border),
      padding: strut(layout.padding),
      obscuring: Rect {
        top: obscures(border.width.top, border.color.top.0, border.style.top),
        right: obscures(border.width.right, border.color.right.0, border.style.right),
        bottom: obscures(
          border.width.bottom,
          border.color.bottom.0,
          border.style.bottom,
        ),
        left: obscures(border.width.left, border.color.left.0, border.style.left),
      },
      disallow_border_derived_adjustment: style.border_collapse == BorderCollapse::Collapse,
    }
  }

  /// The context a mask layer of the box of `size` at `paint_offset` sees: its border box alone,
  /// as `mask-clip` and `mask-origin` both default to it.
  pub fn mask(size: Size<f32>, paint_offset: Point<f32>) -> Self {
    Self {
      paint_offset,
      border_box: UnitRect::nearest(paint_offset, size),
      border: BoxStrut::default(),
      padding: BoxStrut::default(),
      obscuring: Rect {
        top: false,
        right: false,
        bottom: false,
        left: false,
      },
      disallow_border_derived_adjustment: false,
    }
  }

  /// Blink's `ContouredBorderGeometry::PixelSnappedContouredInnerBorder(...).Rect()`.
  fn pixel_snapped_inner_border(&self, border_rect: UnitRect) -> UnitRect {
    let rect_with_outsets = border_rect
      .contract(self.border)
      .clamp_negative_size_to_zero();

    UnitRect {
      offset: UnitOffset {
        left: LayoutUnit::from_int(rect_with_outsets.x().round()),
        top: LayoutUnit::from_int(rect_with_outsets.y().round()),
      },
      size: UnitSize {
        width: LayoutUnit::from_int(snap_size_to_pixel_allowing_zero(
          rect_with_outsets.width(),
          rect_with_outsets.x(),
        )),
        height: LayoutUnit::from_int(snap_size_to_pixel_allowing_zero(
          rect_with_outsets.height(),
          rect_with_outsets.y(),
        )),
      },
    }
  }

  /// Blink's `InnerBorderOutsets`.
  fn inner_border_outsets(&self, dest_rect: UnitRect, positioning_area: UnitRect) -> BoxStrut {
    let inner_border_rect = self.pixel_snapped_inner_border(positioning_area);

    BoxStrut {
      left: inner_border_rect.x() - dest_rect.x(),
      top: inner_border_rect.y() - dest_rect.y(),
      right: dest_rect.right() - inner_border_rect.right(),
      bottom: dest_rect.bottom() - inner_border_rect.bottom(),
    }
  }

  /// Blink's `ObscuredBorderOutsets`.
  fn obscured_border_outsets(
    &self,
    dest_rect: UnitRect,
    positioning_area: UnitRect,
  ) -> SnappedAndUnsnappedOutsets {
    let inner_border_rect = self.pixel_snapped_inner_border(positioning_area);
    let mut adjust = SnappedAndUnsnappedOutsets::default();

    if self.obscuring.top {
      adjust.snapped.top = inner_border_rect.y() - dest_rect.y();
      adjust.unsnapped.top = self.border.top;
    }
    if self.obscuring.right {
      adjust.snapped.right = dest_rect.right() - inner_border_rect.right();
      adjust.unsnapped.right = self.border.right;
    }
    if self.obscuring.bottom {
      adjust.snapped.bottom = dest_rect.bottom() - inner_border_rect.bottom();
      adjust.unsnapped.bottom = self.border.bottom;
    }
    if self.obscuring.left {
      adjust.snapped.left = inner_border_rect.x() - dest_rect.x();
      adjust.unsnapped.left = self.border.left;
    }

    adjust
  }
}

/// Blink's `BorderEdge::ObscuresBackground` for a present edge.
fn obscures_background(width: i32, opaque: bool, style: BorderStyle) -> bool {
  // Blink's `BorderEdge::EffectiveStyle`.
  let style = match style {
    BorderStyle::Double if width < 3 => BorderStyle::Solid,
    BorderStyle::Ridge | BorderStyle::Groove if width <= 1 => BorderStyle::Solid,
    style => style,
  };

  opaque
    && !matches!(
      style,
      BorderStyle::Hidden | BorderStyle::Dotted | BorderStyle::Dashed | BorderStyle::Double
    )
}

/// Blink's `BackgroundImageGeometry`: where one layer paints, in layout units relative to the
/// border box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct BackgroundImageGeometry {
  unsnapped_dest_rect: UnitRect,
  snapped_dest_rect: UnitRect,
  phase: UnitOffset,
  tile_size: UnitSize,
  repeat_spacing: UnitSize,
}

impl BackgroundImageGeometry {
  /// Blink's `Calculate` for a layer that is not `fixed` and a box with no `border-shape`.
  fn calculate(
    fill_layer: &FillLayer,
    paint_context: &BoxBackgroundPaintContext,
    paint_rect: UnitRect,
    sizing: &SizingContext,
  ) -> Self {
    let mut geometry = Self {
      unsnapped_dest_rect: paint_rect,
      ..Self::default()
    };
    let mut unsnapped_positioning_area = paint_rect;
    let mut snapped_positioning_area = UnitRect::default();
    let mut unsnapped_box_offset = UnitOffset::default();
    let mut snapped_box_offset = UnitOffset::default();

    geometry.adjust_positioning_area(
      fill_layer,
      paint_context,
      &mut unsnapped_positioning_area,
      &mut snapped_positioning_area,
      &mut unsnapped_box_offset,
      &mut snapped_box_offset,
    );
    geometry.calculate_fill_tile_size(
      fill_layer,
      unsnapped_positioning_area.size,
      snapped_positioning_area.size,
      sizing,
    );
    geometry.calculate_repeat_and_position(
      fill_layer,
      UnitOffset::default(),
      unsnapped_positioning_area.size,
      snapped_positioning_area.size,
      unsnapped_box_offset,
      snapped_box_offset,
      sizing,
    );

    geometry.unsnapped_dest_rect = geometry.unsnapped_dest_rect.intersect(paint_rect);
    geometry.snapped_dest_rect = geometry.snapped_dest_rect.intersect(paint_rect);
    geometry.snapped_dest_rect = geometry.snapped_dest_rect.pixel_snapped();
    geometry
  }

  fn set_no_repeat_x(&mut self, x_offset: LayoutUnit, snapped_x_offset: LayoutUnit) {
    if x_offset > LayoutUnit::ZERO {
      self.unsnapped_dest_rect.offset.left += x_offset;
      self.snapped_dest_rect.offset.left =
        LayoutUnit::from_int(self.unsnapped_dest_rect.x().round());
      self.unsnapped_dest_rect.size.width = self.tile_size.width;
      self.snapped_dest_rect.size.width = self.tile_size.width;
      self.phase.left = LayoutUnit::ZERO;
    } else {
      self.phase.left = -x_offset;
      self.unsnapped_dest_rect.size.width = self.tile_size.width + x_offset;
      self.snapped_dest_rect.size.width = self.tile_size.width + snapped_x_offset;
    }

    self.repeat_spacing.width = LayoutUnit::ZERO;
  }

  fn set_no_repeat_y(&mut self, y_offset: LayoutUnit, snapped_y_offset: LayoutUnit) {
    if y_offset > LayoutUnit::ZERO {
      self.unsnapped_dest_rect.offset.top += y_offset;
      self.snapped_dest_rect.offset.top =
        LayoutUnit::from_int(self.unsnapped_dest_rect.y().round());
      self.unsnapped_dest_rect.size.height = self.tile_size.height;
      self.snapped_dest_rect.size.height = self.tile_size.height;
      self.phase.top = LayoutUnit::ZERO;
    } else {
      self.phase.top = -y_offset;
      self.unsnapped_dest_rect.size.height = self.tile_size.height + y_offset;
      self.snapped_dest_rect.size.height = self.tile_size.height + snapped_y_offset;
    }

    self.repeat_spacing.height = LayoutUnit::ZERO;
  }

  fn set_repeat_x(&mut self, x_offset: LayoutUnit) {
    self.phase.left = compute_tile_phase(x_offset, self.tile_size.width);
    self.repeat_spacing.width = LayoutUnit::ZERO;
  }

  fn set_repeat_y(&mut self, y_offset: LayoutUnit) {
    self.phase.top = compute_tile_phase(y_offset, self.tile_size.height);
    self.repeat_spacing.height = LayoutUnit::ZERO;
  }

  fn set_space_x(&mut self, space: LayoutUnit, extra_offset: LayoutUnit) {
    self.repeat_spacing.width = space;
    self.phase.left = compute_tile_phase(extra_offset, self.tile_size.width + space);
  }

  fn set_space_y(&mut self, space: LayoutUnit, extra_offset: LayoutUnit) {
    self.repeat_spacing.height = space;
    self.phase.top = compute_tile_phase(extra_offset, self.tile_size.height + space);
  }

  fn compute_dest_rect_adjustments(
    &self,
    fill_layer: &FillLayer,
    paint_context: &BoxBackgroundPaintContext,
    unsnapped_positioning_area: UnitRect,
    disallow_border_derived_adjustment: bool,
  ) -> SnappedAndUnsnappedOutsets {
    let mut dest_adjust = SnappedAndUnsnappedOutsets::default();

    match fill_layer.clip {
      FillBox::Content if !paint_context.padding.is_zero() => {
        dest_adjust.unsnapped = paint_context.padding + paint_context.border;
        dest_adjust.snapped = dest_adjust.unsnapped;
      }
      FillBox::Content | FillBox::Padding => {
        dest_adjust.unsnapped = paint_context.border;
        dest_adjust.snapped = if disallow_border_derived_adjustment {
          dest_adjust.unsnapped
        } else {
          paint_context.inner_border_outsets(self.unsnapped_dest_rect, unsnapped_positioning_area)
        };
      }
      FillBox::Border => {
        if !disallow_border_derived_adjustment {
          dest_adjust = paint_context
            .obscured_border_outsets(self.unsnapped_dest_rect, unsnapped_positioning_area);
        }
      }
      FillBox::Text | FillBox::BorderArea => {}
    }

    dest_adjust
  }

  fn compute_positioning_area_adjustments(
    fill_layer: &FillLayer,
    paint_context: &BoxBackgroundPaintContext,
    unsnapped_positioning_area: UnitRect,
    disallow_border_derived_adjustment: bool,
  ) -> SnappedAndUnsnappedOutsets {
    let mut box_outset = SnappedAndUnsnappedOutsets::default();

    match fill_layer.origin {
      FillBox::Content if !paint_context.padding.is_zero() => {
        box_outset.unsnapped = paint_context.padding + paint_context.border;
        box_outset.snapped = box_outset.unsnapped;
      }
      FillBox::Content | FillBox::Padding => {
        box_outset.unsnapped = paint_context.border;
        box_outset.snapped = if disallow_border_derived_adjustment {
          box_outset.unsnapped
        } else {
          paint_context.inner_border_outsets(unsnapped_positioning_area, unsnapped_positioning_area)
        };
      }
      FillBox::Border | FillBox::Text | FillBox::BorderArea => {}
    }

    box_outset
  }

  fn adjust_positioning_area(
    &mut self,
    fill_layer: &FillLayer,
    paint_context: &BoxBackgroundPaintContext,
    unsnapped_positioning_area: &mut UnitRect,
    snapped_positioning_area: &mut UnitRect,
    unsnapped_box_offset: &mut UnitOffset,
    snapped_box_offset: &mut UnitOffset,
  ) {
    let disallow_border_derived_adjustment =
      fill_layer.kind == FillLayerType::Mask || paint_context.disallow_border_derived_adjustment;
    let dest_adjust = self.compute_dest_rect_adjustments(
      fill_layer,
      paint_context,
      *unsnapped_positioning_area,
      disallow_border_derived_adjustment,
    );
    let box_outset = Self::compute_positioning_area_adjustments(
      fill_layer,
      paint_context,
      *unsnapped_positioning_area,
      disallow_border_derived_adjustment,
    );

    *unsnapped_box_offset = box_outset.unsnapped.offset() - dest_adjust.unsnapped.offset();
    *snapped_box_offset = box_outset.snapped.offset() - dest_adjust.snapped.offset();

    self.snapped_dest_rect = self.unsnapped_dest_rect.contract(dest_adjust.snapped);
    self.snapped_dest_rect = self.snapped_dest_rect.pixel_snapped();
    self.snapped_dest_rect = self.snapped_dest_rect.clamp_negative_size_to_zero();
    self.unsnapped_dest_rect = self
      .unsnapped_dest_rect
      .contract(dest_adjust.unsnapped)
      .clamp_negative_size_to_zero();
    *snapped_positioning_area = unsnapped_positioning_area.contract(box_outset.snapped);
    *snapped_positioning_area = snapped_positioning_area.pixel_snapped();
    *snapped_positioning_area = snapped_positioning_area.clamp_negative_size_to_zero();
    *unsnapped_positioning_area = unsnapped_positioning_area
      .contract(box_outset.unsnapped)
      .clamp_negative_size_to_zero();
  }

  fn calculate_fill_tile_size(
    &mut self,
    fill_layer: &FillLayer,
    unsnapped_positioning_area_size: UnitSize,
    snapped_positioning_area_size: UnitSize,
    sizing: &SizingContext,
  ) {
    let image = &fill_layer.image;
    let sizing_info = image.natural;
    let image_aspect_ratio = sizing_info
      .aspect_ratio
      .map(UnitSize::from_size_floor)
      .unwrap_or_default();
    let positioning_area_size = if !image.has_intrinsic_size() {
      snapped_positioning_area_size
    } else {
      unsnapped_positioning_area_size
    };

    match fill_layer.size {
      BackgroundSize::Explicit {
        width: layer_width,
        height: layer_height,
      } => {
        self.tile_size = positioning_area_size;

        if layer_width != Length::Auto {
          self.tile_size.width = layer_width.value_for(sizing, positioning_area_size.width);
        }
        if layer_height != Length::Auto {
          self.tile_size.height = layer_height.value_for(sizing, positioning_area_size.height);
        }

        match (layer_width == Length::Auto, layer_height == Length::Auto) {
          (true, false) => {
            self.tile_size.width = if !image_aspect_ratio.is_empty() {
              resolve_clamped_width_for_ratio(self.tile_size.height, image_aspect_ratio)
            } else if let Some(width) = sizing_info.width {
              LayoutUnit::from_f32_floor(width)
            } else {
              positioning_area_size.width
            };
          }
          (false, true) => {
            self.tile_size.height = if !image_aspect_ratio.is_empty() {
              resolve_clamped_height_for_ratio(self.tile_size.width, image_aspect_ratio)
            } else if let Some(height) = sizing_info.height {
              LayoutUnit::from_f32_floor(height)
            } else {
              positioning_area_size.height
            };
          }
          (true, true) => {
            self.tile_size =
              UnitSize::from_size_floor(image.image_size(positioning_area_size.to_size()));
          }
          (false, false) => {}
        }

        self.tile_size = self.tile_size.clamp_negative_to_zero();
      }
      BackgroundSize::Contain | BackgroundSize::Cover => {
        if image_aspect_ratio.is_empty() {
          self.tile_size = snapped_positioning_area_size;
          return;
        }

        let cover = fill_layer.size == BackgroundSize::Cover;

        self.tile_size =
          snapped_positioning_area_size.fit_to_aspect_ratio(image_aspect_ratio, cover);
        if !cover {
          if self.tile_size.width != snapped_positioning_area_size.width {
            self.tile_size.width = LayoutUnit::from_int(self.tile_size.width.round().max(1));
          }
          if self.tile_size.height != snapped_positioning_area_size.height {
            self.tile_size.height = LayoutUnit::from_int(self.tile_size.height.round().max(1));
          }
        } else {
          if self.tile_size.width != snapped_positioning_area_size.width {
            self.tile_size.width = self.tile_size.width.max(LayoutUnit::from_int(1));
          }
          if self.tile_size.height != snapped_positioning_area_size.height {
            self.tile_size.height = self.tile_size.height.max(LayoutUnit::from_int(1));
          }
        }
      }
    }
  }

  #[allow(clippy::too_many_arguments)]
  fn calculate_repeat_and_position(
    &mut self,
    fill_layer: &FillLayer,
    offset_in_background: UnitOffset,
    unsnapped_positioning_area_size: UnitSize,
    snapped_positioning_area_size: UnitSize,
    unsnapped_box_offset: UnitOffset,
    snapped_box_offset: UnitOffset,
    sizing: &SizingContext,
  ) {
    let BackgroundRepeat(mut background_repeat_x, mut background_repeat_y) = fill_layer.repeat;
    let (size_width, size_height) = fill_layer.size_length();
    let resolve_x = |available_width: LayoutUnit| {
      fill_layer
        .position_x()
        .minimum_value_for(sizing, available_width)
        - offset_in_background.left
    };
    let resolve_y = |available_height: LayoutUnit| {
      fill_layer
        .position_y()
        .minimum_value_for(sizing, available_height)
        - offset_in_background.top
    };

    let unsnapped_available_width = unsnapped_positioning_area_size.width - self.tile_size.width;
    let unsnapped_available_height = unsnapped_positioning_area_size.height - self.tile_size.height;
    let snapped_available_width = snapped_positioning_area_size.width - self.tile_size.width;
    let snapped_available_height = snapped_positioning_area_size.height - self.tile_size.height;

    if background_repeat_x == BackgroundRepeatStyle::Round
      && snapped_positioning_area_size.width > LayoutUnit::ZERO
      && self.tile_size.width > LayoutUnit::ZERO
    {
      let rounded_width =
        compute_rounded_tile_size(snapped_positioning_area_size.width, self.tile_size.width);

      if size_height == Length::Auto && background_repeat_y != BackgroundRepeatStyle::Round {
        self.tile_size.height = resolve_clamped_height_for_ratio(rounded_width, self.tile_size);
      }
      self.tile_size.width = rounded_width;

      let x_offset = resolve_x(snapped_available_width);

      self.phase.left =
        compute_tile_phase(x_offset + unsnapped_box_offset.left, self.tile_size.width);
      self.repeat_spacing = UnitSize::default();
    }

    if background_repeat_y == BackgroundRepeatStyle::Round
      && snapped_positioning_area_size.height > LayoutUnit::ZERO
      && self.tile_size.height > LayoutUnit::ZERO
    {
      let rounded_height =
        compute_rounded_tile_size(snapped_positioning_area_size.height, self.tile_size.height);

      if size_width == Length::Auto && background_repeat_x != BackgroundRepeatStyle::Round {
        self.tile_size.width = resolve_clamped_width_for_ratio(rounded_height, self.tile_size);
      }
      self.tile_size.height = rounded_height;

      let y_offset = resolve_y(snapped_available_height);

      self.phase.top =
        compute_tile_phase(y_offset + unsnapped_box_offset.top, self.tile_size.height);
      self.repeat_spacing = UnitSize::default();
    }

    if background_repeat_x == BackgroundRepeatStyle::Repeat {
      let x_offset = resolve_x(unsnapped_available_width);

      self.set_repeat_x(unsnapped_box_offset.left + x_offset);
    } else if background_repeat_x == BackgroundRepeatStyle::Space
      && self.tile_size.width > LayoutUnit::ZERO
    {
      let space =
        get_space_between_image_tiles(snapped_positioning_area_size.width, self.tile_size.width);

      if space >= LayoutUnit::ZERO {
        self.set_space_x(space, snapped_box_offset.left);
      } else {
        background_repeat_x = BackgroundRepeatStyle::NoRepeat;
      }
    }
    if background_repeat_x == BackgroundRepeatStyle::NoRepeat {
      let x_offset = resolve_x(unsnapped_available_width);
      let snapped_x_offset = resolve_x(snapped_available_width);

      self.set_no_repeat_x(
        unsnapped_box_offset.left + x_offset,
        snapped_box_offset.left + snapped_x_offset,
      );
    }

    if background_repeat_y == BackgroundRepeatStyle::Repeat {
      let y_offset = resolve_y(unsnapped_available_height);

      self.set_repeat_y(unsnapped_box_offset.top + y_offset);
    } else if background_repeat_y == BackgroundRepeatStyle::Space
      && self.tile_size.height > LayoutUnit::ZERO
    {
      let space =
        get_space_between_image_tiles(snapped_positioning_area_size.height, self.tile_size.height);

      if space >= LayoutUnit::ZERO {
        self.set_space_y(space, snapped_box_offset.top);
      } else {
        background_repeat_y = BackgroundRepeatStyle::NoRepeat;
      }
    }
    if background_repeat_y == BackgroundRepeatStyle::NoRepeat {
      let y_offset = resolve_y(unsnapped_available_height);
      let snapped_y_offset = resolve_y(snapped_available_height);

      self.set_no_repeat_y(
        unsnapped_box_offset.top + y_offset,
        snapped_box_offset.top + snapped_y_offset,
      );
    }
  }

  /// Blink's `ComputePhase`: the phase kept within one tile and its spacing.
  fn compute_phase(&self) -> UnitOffset {
    let step_per_tile = self.tile_size + self.repeat_spacing;

    UnitOffset {
      left: (-self.phase.left).int_mod(step_per_tile.width),
      top: (-self.phase.top).int_mod(step_per_tile.height),
    }
  }

  /// Where `DrawTiledBackground` and `OptimizeToSingleTileDraw` land the tiles of `image`, or
  /// `None` where `PaintFillLayerBackground` paints nothing.
  fn tiling(&self, image: &LayerImage) -> Option<ImageTiling> {
    if self.snapped_dest_rect.is_empty() || self.tile_size.is_empty() {
      return None;
    }

    let snapped_dest = self.snapped_dest_rect;
    let phase = snapped_dest.offset + self.compute_phase();
    let dest = snapped_dest.to_rect();
    let dest_rect_for_subset = UnitRect {
      offset: snapped_dest.offset,
      size: self.unsnapped_dest_rect.size,
    };
    let one_tile_rect = UnitRect {
      offset: phase,
      size: self.tile_size,
    };

    if one_tile_rect.contains(dest_rect_for_subset) {
      return Some(self.single_tile(image, one_tile_rect, dest_rect_for_subset, dest));
    }

    let tile_dest_diff = self.tile_size - snapped_dest.size;
    let half = LayoutUnit::from_raw(32);
    let ref_tile_width = if tile_dest_diff.width.abs() <= half {
      snapped_dest.width()
    } else {
      self.tile_size.width
    };
    let ref_tile_height = if tile_dest_diff.height.abs() <= half {
      snapped_dest.height()
    } else {
      self.tile_size.height
    };

    Some(ImageTiling {
      dest,
      phase: phase.to_point(),
      tile: Size {
        width: ref_tile_width.to_f32(),
        height: ref_tile_height.to_f32(),
      },
      spacing: self.repeat_spacing.to_size(),
    })
  }

  /// `OptimizeToSingleTileDraw`: the one tile `one_tile_rect` that covers `dest_rect`, drawn from
  /// the source subset under `dest_rect_for_subset` into `dest`.
  fn single_tile(
    &self,
    image: &LayerImage,
    one_tile_rect: UnitRect,
    dest_rect_for_subset: UnitRect,
    dest: Rect<f32>,
  ) -> ImageTiling {
    let offset_in_tile = dest_rect_for_subset.offset - one_tile_rect.offset;
    let tile_size = self.tile_size.to_size();

    if !image.has_intrinsic_size() {
      return ImageTiling {
        dest,
        phase: Point {
          x: dest.left - offset_in_tile.left.to_f32(),
          y: dest.top - offset_in_tile.top.to_f32(),
        },
        tile: tile_size,
        spacing: Size::ZERO,
      };
    }

    let intrinsic_tile_size = image.bitmap_size.unwrap_or(tile_size);
    let scale = Size {
      width: tile_size.width / intrinsic_tile_size.width,
      height: tile_size.height / intrinsic_tile_size.height,
    };
    let visible_src_rect = snap_source_rect_if_near_integral(Rect {
      left: offset_in_tile.left.to_f32() / scale.width,
      top: offset_in_tile.top.to_f32() / scale.height,
      right: offset_in_tile.left.to_f32() / scale.width
        + dest_rect_for_subset.width().to_f32() / scale.width,
      bottom: offset_in_tile.top.to_f32() / scale.height
        + dest_rect_for_subset.height().to_f32() / scale.height,
    });
    let dest_per_source = Size {
      width: (dest.right - dest.left) / (visible_src_rect.right - visible_src_rect.left),
      height: (dest.bottom - dest.top) / (visible_src_rect.bottom - visible_src_rect.top),
    };

    ImageTiling {
      dest,
      phase: Point {
        x: dest.left - visible_src_rect.left * dest_per_source.width,
        y: dest.top - visible_src_rect.top * dest_per_source.height,
      },
      tile: Size {
        width: intrinsic_tile_size.width * dest_per_source.width,
        height: intrinsic_tile_size.height * dest_per_source.height,
      },
      spacing: Size::ZERO,
    }
  }
}

/// Blink's `SnapSourceRectIfNearIntegral`.
fn snap_source_rect_if_near_integral(src_rect: Rect<f32>) -> Rect<f32> {
  let epsilon = 1.0 / 64.0;
  let near = |value: f32| (value.round() - value).abs() <= epsilon;

  if !(near(src_rect.left) && near(src_rect.top) && near(src_rect.right) && near(src_rect.bottom)) {
    return src_rect;
  }

  let rounded = Rect {
    left: src_rect.left.round(),
    top: src_rect.top.round(),
    right: src_rect.right.round(),
    bottom: src_rect.bottom.round(),
  };

  if rounded.right <= rounded.left || rounded.bottom <= rounded.top {
    return src_rect;
  }

  rounded
}

/// Blink's `GetSpaceBetweenImageTiles`.
fn get_space_between_image_tiles(area_size: LayoutUnit, tile_size: LayoutUnit) -> LayoutUnit {
  let number_of_tiles = (area_size / tile_size).to_int();

  if number_of_tiles > 1 {
    return (area_size - number_of_tiles * tile_size) / (number_of_tiles - 1);
  }

  LayoutUnit::from_int(-1)
}

/// Blink's `ComputeRoundedTileSize`.
fn compute_rounded_tile_size(area_size: LayoutUnit, tile_size: LayoutUnit) -> LayoutUnit {
  let nr_tiles = (area_size / tile_size).round().max(1);

  area_size / nr_tiles
}

/// Blink's `ComputeTilePhase`.
fn compute_tile_phase(position: LayoutUnit, tile_extent: LayoutUnit) -> LayoutUnit {
  if tile_extent == LayoutUnit::ZERO {
    return LayoutUnit::ZERO;
  }

  tile_extent - position.int_mod(tile_extent)
}

/// Blink's `ResolveClampedWidthForRatio`.
fn resolve_clamped_width_for_ratio(height: LayoutUnit, natural_ratio: UnitSize) -> LayoutUnit {
  let resolved_width = height.mul_div(natural_ratio.width, natural_ratio.height);
  let one = LayoutUnit::from_int(1);

  if natural_ratio.width >= one && resolved_width < one {
    return one;
  }

  resolved_width
}

/// Blink's `ResolveClampedHeightForRatio`.
fn resolve_clamped_height_for_ratio(width: LayoutUnit, natural_ratio: UnitSize) -> LayoutUnit {
  let resolved_height = width.mul_div(natural_ratio.height, natural_ratio.width);
  let one = LayoutUnit::from_int(1);

  if natural_ratio.height >= one && resolved_height < one {
    return one;
  }

  resolved_height
}

/// Blink's `ImageTilingInfo` as the background painters fill it: tiles of `tile` px, one with its
/// top-left at `phase` and the rest `tile + spacing` apart on both axes, showing only inside
/// `dest`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageTiling {
  /// The rectangle the layer paints into, relative to the border box.
  pub dest: Rect<f32>,
  /// The top-left of one tile.
  pub phase: Point<f32>,
  /// The size one tile draws at.
  pub tile: Size<f32>,
  /// The gap between neighbouring tiles.
  pub spacing: Size<f32>,
}

impl ImageTiling {
  /// Distance between the origins of neighbouring tiles.
  pub fn step(&self) -> Size<f32> {
    Size {
      width: self.tile.width + self.spacing.width,
      height: self.tile.height + self.spacing.height,
    }
  }

  /// The tiling moved by `offset`.
  fn shifted(self, offset: Point<f32>) -> Self {
    Self {
      dest: Rect {
        left: self.dest.left + offset.x,
        top: self.dest.top + offset.y,
        right: self.dest.right + offset.x,
        bottom: self.dest.bottom + offset.y,
      },
      phase: Point {
        x: self.phase.x + offset.x,
        y: self.phase.y + offset.y,
      },
      ..self
    }
  }

  /// Whether `dest` covers all of `area`, so the layer needs no clip of its own there.
  pub fn covers(&self, area: Rect<f32>) -> bool {
    self.dest.left <= area.left
      && self.dest.top <= area.top
      && self.dest.right >= area.right
      && self.dest.bottom >= area.bottom
  }

  /// The tile origins on each axis whose tiles meet `dest`.
  pub fn origins(&self) -> (SmallVec<[f32; 1]>, SmallVec<[f32; 1]>) {
    let step = self.step();

    (
      axis_origins(
        self.phase.x,
        self.tile.width,
        step.width,
        self.dest.left,
        self.dest.right,
      ),
      axis_origins(
        self.phase.y,
        self.tile.height,
        step.height,
        self.dest.top,
        self.dest.bottom,
      ),
    )
  }
}

/// Origins `phase + k * step` whose tiles of `tile` meet `start..end`.
fn axis_origins(phase: f32, tile: f32, step: f32, start: f32, end: f32) -> SmallVec<[f32; 1]> {
  let phase = f64::from(phase);
  let step = f64::from(step);
  let first = ((f64::from(start) - f64::from(tile) - phase) / step).floor() as i64 + 1;
  let last = ((f64::from(end) - phase) / step).ceil() as i64 - 1;

  (first..=last)
    .map(|index| (phase + index as f64 * step) as f32)
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    style::{BackgroundRepeats, BackgroundSizes, FromCssStr, PositionValues},
    viewport::Viewport,
  };

  fn layer(size: &str, position: &str, repeat: &str, natural: IntrinsicSizing) -> FillLayer {
    FillLayer {
      kind: FillLayerType::Background,
      image: LayerImage {
        natural,
        bitmap_size: None,
      },
      size: BackgroundSizes::from_css_str(size).unwrap()[0],
      position: PositionValues::from_css_str(position).unwrap()[0],
      repeat: BackgroundRepeats::from_css_str(repeat).unwrap()[0],
      clip: FillBox::Border,
      origin: FillBox::Padding,
    }
  }

  fn calculate(fill_layer: &FillLayer, size: Size<f32>) -> BackgroundImageGeometry {
    let paint_context = BoxBackgroundPaintContext::mask(size, Point::ZERO);

    BackgroundImageGeometry::calculate(
      fill_layer,
      &paint_context,
      paint_context.border_box,
      &SizingContext::builder()
        .viewport(Viewport::new((1200, 630)))
        .build(),
    )
  }

  /// `round` rescales the axis it applies to, and an `auto` axis follows the tile's own ratio.
  #[test]
  fn an_auto_axis_follows_the_rounded_one() {
    let geometry = calculate(
      &layer(
        "auto 80px",
        "left top",
        "no-repeat round",
        IntrinsicSizing::from_dimensions(512.0, 512.0),
      ),
      Size {
        width: 1200.0,
        height: 630.0,
      },
    );

    assert_eq!(geometry.tile_size.height, LayoutUnit::from_f32(78.75));
    assert_eq!(geometry.tile_size.width, LayoutUnit::from_f32(78.75));
  }

  /// A tile under one layout unit truncates to empty and paints nothing.
  #[test]
  fn a_tile_under_a_layout_unit_paints_nothing() {
    let fill_layer = layer(
      "0.001px 1px",
      "left top",
      "repeat",
      IntrinsicSizing::default(),
    );
    let geometry = calculate(
      &fill_layer,
      Size {
        width: 1000.0,
        height: 100.0,
      },
    );

    assert_eq!(geometry.tile_size.width, LayoutUnit::ZERO);
    assert_eq!(geometry.tiling(&fill_layer.image), None);
  }

  /// A repeating tile is placed by its phase, so the tile at the position lands exactly there.
  #[test]
  fn a_repeating_tile_lands_on_its_position() {
    let fill_layer = layer("30px 30px", "10px 0", "repeat", IntrinsicSizing::default());
    let tiling = calculate(
      &fill_layer,
      Size {
        width: 100.0,
        height: 30.0,
      },
    )
    .tiling(&fill_layer.image)
    .unwrap();
    let (xs, ys) = tiling.origins();

    assert_eq!(xs.as_slice(), [-20.0, 10.0, 40.0, 70.0]);
    assert_eq!(ys.as_slice(), [0.0]);
  }

  /// `space` spreads the leftover between whole tiles flush with both edges.
  #[test]
  fn space_spreads_the_leftover() {
    let fill_layer = layer(
      "30px 30px",
      "left top",
      "space no-repeat",
      IntrinsicSizing::default(),
    );
    let tiling = calculate(
      &fill_layer,
      Size {
        width: 100.0,
        height: 30.0,
      },
    )
    .tiling(&fill_layer.image)
    .unwrap();

    assert_eq!(tiling.origins().0.as_slice(), [0.0, 35.0, 70.0]);
  }

  /// A lone tile past the far edge shows nowhere.
  #[test]
  fn a_lone_tile_outside_the_box_paints_nothing() {
    let fill_layer = layer(
      "20px 20px",
      "150px 0",
      "no-repeat",
      IntrinsicSizing::default(),
    );
    let geometry = calculate(
      &fill_layer,
      Size {
        width: 100.0,
        height: 100.0,
      },
    );

    assert_eq!(geometry.tiling(&fill_layer.image), None);
  }
}
