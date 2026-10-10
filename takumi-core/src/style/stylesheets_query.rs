use std::{borrow::Cow, marker::PhantomData};

use taffy::{LengthPercentage, Line, Rect, Size};

use super::ComputedStyle;
use crate::{
  geometry::{Point, Size as CoreSize},
  style::{Lang, SizingContext, properties::*},
};

impl ComputedStyle {
  /// The style a render's root inherits from: its language and font fallback chain.
  pub fn root(lang: Option<Lang>, font_family: Option<FontFamily>) -> Self {
    let mut style = Self {
      lang,
      ..Self::default()
    };

    if let Some(font_family) = font_family {
      style.inherited_data_mut().font_family = font_family;
    }
    style
  }

  /// Normalize inheritable text-related values to computed values for this node.
  pub(crate) fn make_computed(&mut self, sizing: &SizingContext) {
    #[cfg(debug_assertions)]
    let expected = {
      let mut style = self.unshared();

      style.resolve_computed(sizing);
      style
    };

    self.share_computed_initial_groups();
    self.resolve_computed(sizing);
    #[cfg(debug_assertions)]
    debug_assert!(
      *self == expected,
      "a shared style group resolved differently"
    );
  }

  /// [`Self::make_computed`] on the groups this style owns alone, writing a shared group only
  /// when its value changes.
  pub(super) fn resolve_computed(&mut self, sizing: &SizingContext) {
    // `font-size` computed value is already resolved in `sizing.font_size`.
    // Keep it as css-px in style to avoid re-resolving descendant inheritance.
    let font_size = FontSize::Length(Length::Px(sizing.to_css(sizing.font_size)));

    if self.inherited_data.font_size != font_size {
      self.inherited_data_mut().font_size = font_size;
    }

    self.make_computed_values(sizing);

    // The used value of `border-width`/`outline-width` is zero when the line's
    // style is `none` or `hidden`, even though the computed value is `medium`.
    let none = LineWidth::Length(Length::zero());
    let box_data = &self.box_data;
    let zeroed = [
      !box_data.border_top_style.is_rendered() && box_data.border_top_width != none,
      !box_data.border_right_style.is_rendered() && box_data.border_right_width != none,
      !box_data.border_bottom_style.is_rendered() && box_data.border_bottom_width != none,
      !box_data.border_left_style.is_rendered() && box_data.border_left_width != none,
    ];

    if zeroed.contains(&true) {
      let box_data = self.box_data_mut();
      let widths = [
        &mut box_data.border_top_width,
        &mut box_data.border_right_width,
        &mut box_data.border_bottom_width,
        &mut box_data.border_left_width,
      ];

      for (width, zeroed) in widths.into_iter().zip(zeroed) {
        if zeroed {
          *width = none;
        }
      }
    }
    if !self.rare_non_inherited_data.outline_style.is_rendered()
      && self.rare_non_inherited_data.outline_width != none
    {
      self.rare_non_inherited_data_mut().outline_width = none;
    }

    self.original_display = self.base_data.display;

    // https://www.w3.org/TR/css-display-3/#transformations
    // Elements with position: absolute or fixed are blockified
    let base_data = &self.base_data;

    if (base_data.position.is_out_of_flow() || base_data.float != Float::None)
      && base_data.display.as_blockified() != base_data.display
    {
      self.base_data_mut().display.blockify();
    }
  }

  /// Whether the element and everything inside it paint nothing: zero opacity or `display:none`.
  /// A `visibility: hidden` element still lets its visible descendants paint.
  pub fn is_invisible(&self) -> bool {
    self.svg_data.opacity.0 == 0.0 || self.base_data.display == Display::None
  }

  /// Whether the element paints its own box and content, which `visibility: hidden` stops.
  pub(crate) fn is_visible(&self) -> bool {
    self.inherited_data.visibility == Visibility::Visible
  }

  /// Whether `z-index` takes effect on this element.
  pub(crate) fn is_z_index_applicable(&self, is_flex_or_grid_item: bool) -> bool {
    !matches!(self.box_data.z_index, ZIndex::Auto)
      && (self.base_data.position.is_positioned() || is_flex_or_grid_item)
  }

  /// The effective paint-order z-index of a child with this style.
  pub(crate) fn paint_order_z(&self, is_flex_or_grid_item: bool) -> i32 {
    if self.participates_in_positioned_paint_bucket(is_flex_or_grid_item) {
      self.box_data.z_index.painting_order_value()
    } else {
      0
    }
  }

  /// Whether the element paints in the positioned/z-index bucket.
  pub(crate) fn participates_in_positioned_paint_bucket(&self, is_flex_or_grid_item: bool) -> bool {
    self.base_data.position.is_positioned() || self.is_z_index_applicable(is_flex_or_grid_item)
  }

  /// Whether the element establishes a new stacking context, as Blink's
  /// `ComputedStyle::CalculateIsStackingContextWithoutContainment` decides for the properties
  /// takumi has, plus the containment half of `LayoutObject::IsStackingContext`.
  pub(crate) fn creates_stacking_context(&self, is_flex_or_grid_item: bool) -> bool {
    self.base_data.position == Position::Fixed
      || self
        .rare_non_inherited_data
        .contain
        .contains(Contain::LAYOUT)
      || self
        .rare_non_inherited_data
        .contain
        .contains(Contain::PAINT)
      || self.is_z_index_applicable(is_flex_or_grid_item)
      || self.has_transform_related_property()
      || self.needs_offscreen_compositing()
  }

  /// Whether the element floats in a paint layer of its own, as Blink's floats with a
  /// self-painting layer do.
  pub(crate) fn floats_in_own_layer(&self, is_flex_or_grid_item: bool) -> bool {
    self.base_data.float != Float::None
      && !is_flex_or_grid_item
      && (self.base_data.position.is_positioned()
        || self.creates_stacking_context(is_flex_or_grid_item))
  }

  /// Whether the box is a containing block for `fixed` descendants, and so
  /// also for `absolute` ones. Blink resolves this as `ComputeIsFixedContainer`
  /// (`layout_object.cc`); the conditions takumi has properties for are a
  /// transform-related property, a non-initial `filter` / `backdrop-filter`,
  /// and `layout` or `paint` containment.
  pub fn contains_fixed_descendants(&self) -> bool {
    self
      .rare_non_inherited_data
      .contain
      .contains(Contain::LAYOUT)
      || self
        .rare_non_inherited_data
        .contain
        .contains(Contain::PAINT)
      || self.has_transform_related_property()
      || !self.rare_non_inherited_data.filter.is_empty()
      || !self.rare_non_inherited_data.backdrop_filter.is_empty()
  }

  /// Blink's `HasTransformRelatedProperty`, for the properties takumi has.
  pub(crate) fn has_transform_related_property(&self) -> bool {
    self
      .svg_data
      .transform
      .as_ref()
      .is_some_and(|t| !t.0.is_empty())
      || self.rare_non_inherited_data.offset_path.is_some()
      || self.rare_non_inherited_data.rotate.is_some()
      || self.rare_non_inherited_data.translate != SpacePair::default()
      || self.rare_non_inherited_data.scale.is_some()
  }

  /// Blink's `UpdateForPaintOffsetTranslation`: the paint offset a box with this style paints
  /// at, from `paint_offset` in its parent's space, after the translation its transform, backdrop
  /// filter, or containment takes moves the whole pixels out. `local` is the box's transform.
  pub(crate) fn paint_offset_after_translation(
    &self,
    paint_offset: Point<f32>,
    local: Affine,
  ) -> Point<f32> {
    // Blink's `NeedsIsolationNodes`.
    let isolates = self
      .rare_non_inherited_data
      .contain
      .contains(Contain::PAINT)
      || (self
        .rare_non_inherited_data
        .contain
        .contains(Contain::STYLE)
        && self
          .rare_non_inherited_data
          .contain
          .contains(Contain::LAYOUT));

    if isolates {
      return Point::ZERO;
    }
    if !self.has_transform_related_property()
      && self.rare_non_inherited_data.backdrop_filter.is_empty()
    {
      return paint_offset;
    }

    // `ToRoundedVector2d` rounds as `LayoutUnit::Round` does, halves up.
    let subpixel = |offset: f32| offset - (offset + 0.5).floor();
    // Blink's `CanPropagateSubpixelAccumulation`.
    let (keep_x, keep_y) = if local.only_translation() {
      (true, true)
    } else if local.b == 0.0 && local.c == 0.0 {
      (local.a == 1.0, local.d == 1.0)
    } else {
      (false, false)
    };

    Point {
      x: if keep_x {
        subpixel(paint_offset.x)
      } else {
        0.0
      },
      y: if keep_y {
        subpixel(paint_offset.y)
      } else {
        0.0
      },
    }
  }

  /// Whether the element must render to an offscreen layer before compositing.
  pub fn needs_offscreen_compositing(&self) -> bool {
    self.rare_non_inherited_data.isolation == Isolation::Isolate
      || *self.svg_data.opacity < 1.0
      || !self.rare_non_inherited_data.filter.is_empty()
      || !self.rare_non_inherited_data.backdrop_filter.is_empty()
      || self.rare_non_inherited_data.mix_blend_mode != BlendMode::Normal
      || self.has_shape_mask()
  }

  /// Whether `clip-path` or a non-empty `mask-image` shapes the element's
  /// visible area.
  pub fn has_shape_mask(&self) -> bool {
    self.rare_non_inherited_data.clip_path.is_some()
      || self
        .rare_non_inherited_data
        .mask_image
        .as_ref()
        .is_some_and(|images| {
          images
            .iter()
            .any(|image| !matches!(image, BackgroundImage::None))
        })
  }

  /// Builds the element's local affine transform around its transform-origin
  /// (CSS Transforms Level 2 order: `T(origin) * translate * rotate * scale *
  /// transform * T(-origin)`).
  pub fn local_transform(&self, width: f32, height: f32, sizing: &SizingContext) -> Affine {
    let (origin_x, origin_y) = self
      .svg_data
      .transform_origin
      .to_point(sizing, width, height);
    let mut local = Affine::translation(origin_x, origin_y);

    if self.rare_non_inherited_data.translate != SpacePair::default() {
      local *= Affine::translation(
        self
          .rare_non_inherited_data
          .translate
          .x
          .to_px(sizing, width),
        self
          .rare_non_inherited_data
          .translate
          .y
          .to_px(sizing, height),
      );
    }
    if let Some(rotate) = self.rare_non_inherited_data.rotate {
      local *= Affine::rotation(rotate);
    }
    if let Some(scale) = self.rare_non_inherited_data.scale {
      local *= Affine::scale(scale.x.0, scale.y.0);
    }
    // offset-path sits after translate/rotate/scale and before `transform`, and
    // resolves against the containing block (Blink `GetReferenceBox`), proxied
    // here by the query-container size, then the viewport, then the border box.
    let reference_width = sizing
      .container_size
      .width
      .filter(|width| *width > 0.0)
      .or_else(|| sizing.viewport.size.width.map(|width| width as f32))
      .unwrap_or(width);
    let reference_height = sizing
      .container_size
      .height
      .filter(|height| *height > 0.0)
      .or_else(|| sizing.viewport.size.height.map(|height| height as f32))
      .unwrap_or(height);
    if let Some(path) = &self.rare_non_inherited_data.offset_path
      && let Some((point, tangent)) = path.sample(
        self.rare_non_inherited_data.offset_distance,
        &self.rare_non_inherited_data.offset_position,
        sizing,
        CoreSize {
          width: reference_width,
          height: reference_height,
        },
      )
    {
      local *= Affine::translation(point.x - origin_x, point.y - origin_y);
      local *=
        Affine::rotation_radians(self.rare_non_inherited_data.offset_rotate.resolve(tangent));
      if let Some((anchor_x, anchor_y)) = self
        .rare_non_inherited_data
        .offset_anchor
        .resolve(sizing, width, height)
      {
        local *= Affine::translation(origin_x - anchor_x, origin_y - anchor_y);
      }
    }
    if let Some(node_transform) = &self.svg_data.transform {
      local *= Affine::from_transforms(node_transform.iter(), sizing, width, height);
    }
    local *= Affine::translation(-origin_x, -origin_y);
    local
  }

  /// The computed `(overflow-x, overflow-y)` pair. `visible` paired with an axis
  /// that is neither `visible` nor `clip` computes to a clipping value, per
  /// <https://drafts.csswg.org/css-overflow-3/#overflow-properties>. Blink
  /// resolves it to `auto`; without a scrolling box that is `hidden` here.
  /// `contain: paint` clips a `visible` axis to the padding edge, as Blink's
  /// `LayoutBox::ComputeOverflowClipAxes` does.
  pub fn resolve_overflows(&self) -> SpacePair<Overflow> {
    let (mut x, mut y) = match (self.base_data.overflow_x, self.base_data.overflow_y) {
      (Overflow::Visible, other) if !other.is_clip_or_visible() => (Overflow::Hidden, other),
      (other, Overflow::Visible) if !other.is_clip_or_visible() => (other, Overflow::Hidden),
      pair => pair,
    };

    if self
      .rare_non_inherited_data
      .contain
      .contains(Contain::PAINT)
    {
      if x == Overflow::Visible {
        x = Overflow::Clip;
      }
      if y == Overflow::Visible {
        y = Overflow::Clip;
      }
    }

    SpacePair::from_pair(x, y)
  }

  /// Whether overflowing content is clipped.
  pub fn clips_overflow(&self) -> bool {
    self.resolve_overflows().should_clip_content()
  }

  /// The string used to mark truncated text.
  pub(crate) fn ellipsis_char(&self) -> &str {
    const ELLIPSIS_CHAR: &str = "…";

    match &self.rare_non_inherited_data.text_overflow {
      TextOverflow::Ellipsis => return ELLIPSIS_CHAR,
      TextOverflow::Custom(custom) => return custom.as_str(),
      _ => {}
    }

    match &self.rare_inherited_data.block_ellipsis {
      BlockEllipsis::String(custom) => custom.as_str(),
      BlockEllipsis::None => "",
      BlockEllipsis::Auto => ELLIPSIS_CHAR,
    }
  }

  /// `nowrap` + `ellipsis`: parley lays out all the text even when it overflows,
  /// so this case is rendered by switching to wrapping with a one-line clamp.
  fn forces_single_line_ellipsis(&self) -> bool {
    self.inherited_data.text_wrap_mode == TextWrapMode::NoWrap
      && self.rare_non_inherited_data.text_overflow == TextOverflow::Ellipsis
  }

  /// The wrap mode used for layout, forcing wrap for single-line ellipsis.
  pub(crate) fn resolved_text_wrap_mode(&self) -> TextWrapMode {
    if self.forces_single_line_ellipsis() {
      TextWrapMode::Wrap
    } else {
      self.inherited_data.text_wrap_mode
    }
  }

  /// The number of lines to clamp to for layout, or `None` when not clamped.
  ///
  /// `nowrap` + `ellipsis` clamps to a single line; otherwise `max-lines` applies
  /// only inside a fragmentation context (`continue: collapse`), per CSS Overflow 4.
  /// The ellipsis itself comes from [`Self::ellipsis_char`].
  pub(crate) fn clamp_lines(&self) -> Option<u32> {
    if self.forces_single_line_ellipsis() {
      return Some(1);
    }

    if self.rare_non_inherited_data.r#continue != Continue::Collapse {
      return None;
    }

    self
      .rare_non_inherited_data
      .max_lines
      .filter(|&count| count >= 1)
  }

  #[inline]
  fn grid_template(
    components: &Option<GridTemplateComponents>,
    sizing: &SizingContext,
  ) -> (Vec<taffy::GridTemplateComponent<String>>, Vec<Vec<String>>) {
    components.as_deref().map_or_else(
      || (Vec::new(), vec![Vec::new()]),
      |components| collect_components_and_names(components, sizing),
    )
  }

  #[inline]
  /// The decoration thickness resolved to pixels or `from-font`.
  pub(crate) fn resolved_text_decoration_thickness(
    &self,
    sizing: &SizingContext,
  ) -> SizedTextDecorationThickness {
    match self.rare_non_inherited_data.text_decoration_thickness {
      TextDecorationThickness::Length(Length::Auto) => SizedTextDecorationThickness::Auto,
      TextDecorationThickness::FromFont => SizedTextDecorationThickness::FromFont,
      TextDecorationThickness::Length(thickness) => {
        SizedTextDecorationThickness::Value(thickness.to_px(sizing, sizing.font_size))
      }
    }
  }

  /// Resolved OpenType features: the `font-variant-*` expansions followed by explicit
  /// `font-feature-settings`, which win on tag conflicts. Borrows the settings when no
  /// `font-variant-*` is active, avoiding an allocation.
  pub(crate) fn resolved_font_features(&self) -> Cow<'_, [FontFeature]> {
    let mut features = Vec::new();
    self
      .inherited_data
      .font_variant_ligatures
      .append_features(&mut features);
    self
      .inherited_data
      .font_variant_numeric
      .append_features(&mut features);
    self
      .inherited_data
      .font_variant_east_asian
      .append_features(&mut features);
    self
      .inherited_data
      .font_variant_caps
      .append_features(&mut features);
    self
      .inherited_data
      .font_variant_position
      .append_features(&mut features);
    self
      .inherited_data
      .font_kerning
      .append_features(&mut features);

    if features.is_empty() {
      return Cow::Borrowed(self.inherited_data.font_feature_settings.as_ref());
    }

    features.extend_from_slice(&self.inherited_data.font_feature_settings);
    Cow::Owned(features)
  }

  /// Converts the computed style into a `taffy::Style` for layout.
  pub(crate) fn to_taffy_style(&self, sizing: &SizingContext) -> taffy::Style {
    // Convert grid templates and associated line names
    let (grid_template_columns, grid_template_column_names) =
      Self::grid_template(&self.rare_non_inherited_data.grid_template_columns, sizing);
    let (grid_template_rows, grid_template_row_names) =
      Self::grid_template(&self.rare_non_inherited_data.grid_template_rows, sizing);

    taffy::Style {
      contain: self.rare_non_inherited_data.contain.into_taffy(),
      float: self.base_data.float.resolve(self.inherited_data.direction),
      clear: self.base_data.clear.resolve(self.inherited_data.direction),
      direction: self.inherited_data.direction.into_taffy(),
      box_sizing: self.box_data.box_sizing.into_taffy(),
      size: Size {
        width: self.box_data.width,
        height: self.box_data.height,
      }
      .map(|length| length.resolve_to_dimension(sizing)),
      // Used widths are already zeroed for non-rendered styles in `make_computed`.
      border: Rect {
        top: self.box_data.border_top_width,
        right: self.box_data.border_right_width,
        bottom: self.box_data.border_bottom_width,
        left: self.box_data.border_left_width,
      }
      .map(|border| LengthPercentage::length(border.to_used_px(sizing))),
      padding: Rect {
        top: self.box_data.padding_top,
        right: self.box_data.padding_right,
        bottom: self.box_data.padding_bottom,
        left: self.box_data.padding_left,
      }
      .map(|padding| padding.resolve_to_length_percentage(sizing)),
      inset: if self.base_data.position == Position::Static {
        Rect::auto()
      } else {
        Rect {
          top: self.surround_data.top,
          right: self.surround_data.right,
          bottom: self.surround_data.bottom,
          left: self.surround_data.left,
        }
        .map(|inset| inset.resolve_to_length_percentage_auto(sizing))
      },
      margin: Rect {
        top: self.box_data.margin_top,
        right: self.box_data.margin_right,
        bottom: self.box_data.margin_bottom,
        left: self.box_data.margin_left,
      }
      .map(|margin| margin.resolve_to_length_percentage_auto(sizing)),
      display: self.base_data.display.into_taffy(),
      flex_direction: self.rare_non_inherited_data.flex_direction.into_taffy(),
      position: self.base_data.position.into_taffy(),
      justify_content: self.box_data.justify_content.into_taffy(),
      align_content: self.rare_non_inherited_data.align_content.into_taffy(),
      justify_items: self.rare_non_inherited_data.justify_items.into_taffy(),
      flex_grow: self
        .rare_non_inherited_data
        .flex_grow
        .map(|grow| grow.0)
        .unwrap_or(0.0),
      align_items: self.box_data.align_items.into_taffy(),
      gap: Size {
        width: self
          .rare_non_inherited_data
          .column_gap
          .resolve_to_length_percentage(sizing),
        height: self
          .rare_non_inherited_data
          .row_gap
          .resolve_to_length_percentage(sizing),
      },
      flex_basis: self
        .rare_non_inherited_data
        .flex_basis
        .unwrap_or_default()
        .resolve_to_dimension(sizing),
      flex_shrink: self
        .rare_non_inherited_data
        .flex_shrink
        .map(|shrink| shrink.0)
        .unwrap_or(1.0),
      flex_wrap: self.rare_non_inherited_data.flex_wrap.into_taffy(),
      flex_line_count: self.rare_non_inherited_data.flex_line_count.get(),
      min_size: Size {
        width: self.box_data.min_width,
        height: self.box_data.min_height,
      }
      .map(|length| length.resolve_to_length_percentage_auto(sizing)),
      max_size: Size {
        width: self.box_data.max_width,
        height: self.box_data.max_height,
      }
      .map(|length| length.resolve_to_length_percentage_auto(sizing)),
      grid_auto_columns: self
        .rare_non_inherited_data
        .grid_auto_columns
        .as_ref()
        .map_or_else(Vec::new, |tracks| {
          tracks
            .iter()
            .map(|track| track.to_min_max(sizing))
            .collect()
        }),
      grid_auto_rows: self
        .rare_non_inherited_data
        .grid_auto_rows
        .as_ref()
        .map_or_else(Vec::new, |tracks| {
          tracks
            .iter()
            .map(|track| track.to_min_max(sizing))
            .collect()
        }),
      grid_auto_flow: self.rare_non_inherited_data.grid_auto_flow.into_taffy(),
      grid_column: Line {
        start: self
          .rare_non_inherited_data
          .grid_column_start
          .clone()
          .into_taffy(),
        end: self
          .rare_non_inherited_data
          .grid_column_end
          .clone()
          .into_taffy(),
      },
      grid_row: Line {
        start: self
          .rare_non_inherited_data
          .grid_row_start
          .clone()
          .into_taffy(),
        end: self
          .rare_non_inherited_data
          .grid_row_end
          .clone()
          .into_taffy(),
      },
      grid_template_columns,
      grid_template_rows,
      grid_template_column_names,
      grid_template_row_names,
      grid_template_areas: self
        .rare_non_inherited_data
        .grid_template_areas
        .as_ref()
        .cloned()
        .and_then(GridTemplateAreas::into_taffy),
      aspect_ratio: self.surround_data.aspect_ratio.into(),
      align_self: self.rare_non_inherited_data.align_self.into_taffy(),
      justify_self: self.rare_non_inherited_data.justify_self.into_taffy(),
      overflow: self
        .resolve_overflows()
        .into_taffy()
        .map(Overflow::into_taffy),
      dummy: PhantomData,
      item_is_table: false,
      item_is_replaced: false,
      scrollbar_width: 0.0,
      text_align: taffy::TextAlign::Auto,
    }
  }
}
