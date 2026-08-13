use std::{borrow::Cow, marker::PhantomData};

use taffy::{LengthPercentage, Line, Rect, Size};

use super::ComputedStyle;
use crate::{
  geometry::Size as CoreSize,
  style::{SizingContext, properties::*},
};

impl ComputedStyle {
  /// Normalize inheritable text-related values to computed values for this node.
  pub(crate) fn make_computed(&mut self, sizing: &SizingContext) {
    // `font-size` computed value is already resolved in `sizing.font_size`.
    // Keep it as css-px in style to avoid re-resolving descendant inheritance.
    self.font_size = FontSize::Length(Length::Px(sizing.to_css(sizing.font_size)));

    self.make_computed_values(sizing);

    // The used value of `border-width`/`outline-width` is zero when the line's
    // style is `none` or `hidden`, even though the computed value is `medium`.
    if !self.border_top_style.is_rendered() {
      self.border_top_width = LineWidth::Length(Length::zero());
    }
    if !self.border_right_style.is_rendered() {
      self.border_right_width = LineWidth::Length(Length::zero());
    }
    if !self.border_bottom_style.is_rendered() {
      self.border_bottom_width = LineWidth::Length(Length::zero());
    }
    if !self.border_left_style.is_rendered() {
      self.border_left_width = LineWidth::Length(Length::zero());
    }
    if !self.outline_style.is_rendered() {
      self.outline_width = LineWidth::Length(Length::zero());
    }

    // https://www.w3.org/TR/css-display-3/#transformations
    // Elements with position: absolute or fixed are blockified
    if self.position.is_out_of_flow() || self.float != Float::None {
      self.display.blockify();
    }
  }

  /// Whether the element paints nothing (zero opacity, `display:none`, hidden).
  pub fn is_invisible(&self) -> bool {
    self.opacity.0 == 0.0 || self.display == Display::None || self.visibility == Visibility::Hidden
  }

  /// Whether `z-index` takes effect on this element.
  pub(crate) fn is_z_index_applicable(&self, is_flex_or_grid_item: bool) -> bool {
    !matches!(self.z_index, ZIndex::Auto) && (self.position.is_positioned() || is_flex_or_grid_item)
  }

  /// Whether the element paints in the positioned/z-index bucket.
  pub(crate) fn participates_in_positioned_paint_bucket(&self, is_flex_or_grid_item: bool) -> bool {
    self.position.is_positioned() || self.is_z_index_applicable(is_flex_or_grid_item)
  }

  /// Whether the element establishes a new stacking context.
  pub(crate) fn creates_stacking_context(
    &self,
    width: f32,
    height: f32,
    sizing: &SizingContext,
    is_flex_or_grid_item: bool,
  ) -> bool {
    self.isolation == Isolation::Isolate
      || self.is_z_index_applicable(is_flex_or_grid_item)
      || self.offset_path.is_some()
      || self.has_non_identity_transform(width, height, sizing)
      || self.needs_offscreen_compositing()
  }

  /// Whether the element must render to an offscreen layer before compositing.
  pub fn needs_offscreen_compositing(&self) -> bool {
    self.isolation == Isolation::Isolate
      || *self.opacity < 1.0
      || !self.filter.is_empty()
      || !self.backdrop_filter.is_empty()
      || self.mix_blend_mode != BlendMode::Normal
      || self.has_shape_mask()
  }

  /// Whether `clip-path` or a non-empty `mask-image` shapes the element's
  /// visible area.
  pub fn has_shape_mask(&self) -> bool {
    self.clip_path.is_some()
      || self.mask_image.as_ref().is_some_and(|images| {
        images
          .iter()
          .any(|image| !matches!(image, BackgroundImage::None))
      })
  }

  /// Builds the element's local affine transform around its transform-origin
  /// (CSS Transforms Level 2 order: `T(origin) * translate * rotate * scale *
  /// transform * T(-origin)`).
  pub fn local_transform(&self, width: f32, height: f32, sizing: &SizingContext) -> Affine {
    let (origin_x, origin_y) = self.transform_origin.to_point(sizing, width, height);
    let mut local = Affine::translation(origin_x, origin_y);

    if self.translate != SpacePair::default() {
      local *= Affine::translation(
        self.translate.x.to_px(sizing, width),
        self.translate.y.to_px(sizing, height),
      );
    }
    if let Some(rotate) = self.rotate {
      local *= Affine::rotation(rotate);
    }
    if self.scale != SpacePair::default() {
      local *= Affine::scale(self.scale.x.0, self.scale.y.0);
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
    if let Some(path) = &self.offset_path
      && let Some((point, tangent)) = sample_offset_path(
        path,
        self.offset_distance,
        &self.offset_position,
        sizing,
        CoreSize {
          width: reference_width,
          height: reference_height,
        },
      )
    {
      local *= Affine::translation(point.x - origin_x, point.y - origin_y);
      local *= Affine::rotation_radians(self.offset_rotate.resolve(tangent));
      if let Some((anchor_x, anchor_y)) = self.offset_anchor.resolve(sizing, width, height) {
        local *= Affine::translation(origin_x - anchor_x, origin_y - anchor_y);
      }
    }
    if let Some(node_transform) = &self.transform {
      local *= Affine::from_transforms(node_transform.iter(), sizing, width, height);
    }
    local *= Affine::translation(-origin_x, -origin_y);
    local
  }

  /// Whether the resolved local transform differs from identity.
  pub(crate) fn has_non_identity_transform(
    &self,
    width: f32,
    height: f32,
    sizing: &SizingContext,
  ) -> bool {
    !self.local_transform(width, height, sizing).is_identity()
  }

  /// The computed `(overflow-x, overflow-y)` pair. `visible` paired with an axis
  /// that is neither `visible` nor `clip` computes to a clipping value, per
  /// <https://drafts.csswg.org/css-overflow-3/#overflow-properties>. Blink
  /// resolves it to `auto`; without a scrolling box that is `hidden` here.
  pub fn resolve_overflows(&self) -> SpacePair<Overflow> {
    let (x, y) = match (self.overflow_x, self.overflow_y) {
      (Overflow::Visible, other) if !other.is_clip_or_visible() => (Overflow::Hidden, other),
      (other, Overflow::Visible) if !other.is_clip_or_visible() => (other, Overflow::Hidden),
      pair => pair,
    };

    SpacePair::from_pair(x, y)
  }

  /// Whether overflowing content is clipped.
  pub fn clips_overflow(&self) -> bool {
    self.resolve_overflows().should_clip_content()
  }

  /// The string used to mark truncated text.
  pub(crate) fn ellipsis_char(&self) -> &str {
    const ELLIPSIS_CHAR: &str = "…";

    match &self.text_overflow {
      TextOverflow::Ellipsis => return ELLIPSIS_CHAR,
      TextOverflow::Custom(custom) => return custom.as_str(),
      _ => {}
    }

    match &self.block_ellipsis {
      BlockEllipsis::String(custom) => custom.as_str(),
      BlockEllipsis::None => "",
      BlockEllipsis::Auto => ELLIPSIS_CHAR,
    }
  }

  /// `nowrap` + `ellipsis`: parley lays out all the text even when it overflows,
  /// so this case is rendered by switching to wrapping with a one-line clamp.
  fn forces_single_line_ellipsis(&self) -> bool {
    self.text_wrap_mode == TextWrapMode::NoWrap && self.text_overflow == TextOverflow::Ellipsis
  }

  /// The wrap mode used for layout, forcing wrap for single-line ellipsis.
  pub(crate) fn resolved_text_wrap_mode(&self) -> TextWrapMode {
    if self.forces_single_line_ellipsis() {
      TextWrapMode::Wrap
    } else {
      self.text_wrap_mode
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

    if self.r#continue != Continue::Collapse {
      return None;
    }

    self.max_lines.filter(|&count| count >= 1)
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
    match self.text_decoration_thickness {
      TextDecorationThickness::Length(Length::Auto) | TextDecorationThickness::FromFont => {
        SizedTextDecorationThickness::FromFont
      }
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
    append_variant_features(
      &self.font_variant_ligatures,
      &self.font_variant_numeric,
      &self.font_variant_east_asian,
      &self.font_variant_caps,
      &self.font_variant_position,
      &mut features,
    );
    self.font_kerning.append_features(&mut features);

    if features.is_empty() {
      return Cow::Borrowed(self.font_feature_settings.as_ref());
    }

    features.extend_from_slice(&self.font_feature_settings);
    Cow::Owned(features)
  }

  /// Converts the computed style into a `taffy::Style` for layout.
  pub(crate) fn to_taffy_style(&self, sizing: &SizingContext) -> taffy::Style {
    // Convert grid templates and associated line names
    let (grid_template_columns, grid_template_column_names) =
      Self::grid_template(&self.grid_template_columns, sizing);
    let (grid_template_rows, grid_template_row_names) =
      Self::grid_template(&self.grid_template_rows, sizing);

    taffy::Style {
      float: self.float.resolve(self.direction),
      clear: self.clear.resolve(self.direction),
      direction: self.direction.into_taffy(),
      box_sizing: self.box_sizing.into_taffy(),
      size: Size {
        width: self.width,
        height: self.height,
      }
      .map(|length| length.resolve_to_dimension(sizing)),
      // Used widths are already zeroed for non-rendered styles in `make_computed`.
      border: Rect {
        top: self.border_top_width,
        right: self.border_right_width,
        bottom: self.border_bottom_width,
        left: self.border_left_width,
      }
      .map(|border| LengthPercentage::length(border.to_used_px(sizing))),
      padding: Rect {
        top: self.padding_top,
        right: self.padding_right,
        bottom: self.padding_bottom,
        left: self.padding_left,
      }
      .map(|padding| padding.resolve_to_length_percentage(sizing)),
      inset: if self.position == Position::Static {
        Rect::auto()
      } else {
        Rect {
          top: self.top,
          right: self.right,
          bottom: self.bottom,
          left: self.left,
        }
        .map(|inset| inset.resolve_to_length_percentage_auto(sizing))
      },
      margin: Rect {
        top: self.margin_top,
        right: self.margin_right,
        bottom: self.margin_bottom,
        left: self.margin_left,
      }
      .map(|margin| margin.resolve_to_length_percentage_auto(sizing)),
      display: self.display.into_taffy(),
      flex_direction: self.flex_direction.into_taffy(),
      position: self.position.into_taffy(),
      justify_content: self.justify_content.into_taffy(),
      align_content: self.align_content.into_taffy(),
      justify_items: self.justify_items.into_taffy(),
      flex_grow: self.flex_grow.map(|grow| grow.0).unwrap_or(0.0),
      align_items: self.align_items.into_taffy(),
      gap: Size {
        width: self.column_gap.resolve_to_length_percentage(sizing),
        height: self.row_gap.resolve_to_length_percentage(sizing),
      },
      flex_basis: self
        .flex_basis
        .unwrap_or(Length::Auto)
        .resolve_to_dimension(sizing),
      flex_shrink: self.flex_shrink.map(|shrink| shrink.0).unwrap_or(1.0),
      flex_wrap: self.flex_wrap.into_taffy(),
      min_size: Size {
        width: self.min_width,
        height: self.min_height,
      }
      .map(|length| length.resolve_to_dimension(sizing)),
      max_size: Size {
        width: self.max_width,
        height: self.max_height,
      }
      .map(|length| length.resolve_to_dimension(sizing)),
      grid_auto_columns: self
        .grid_auto_columns
        .as_ref()
        .map_or_else(Vec::new, |tracks| {
          tracks
            .iter()
            .map(|track| track.to_min_max(sizing))
            .collect()
        }),
      grid_auto_rows: self
        .grid_auto_rows
        .as_ref()
        .map_or_else(Vec::new, |tracks| {
          tracks
            .iter()
            .map(|track| track.to_min_max(sizing))
            .collect()
        }),
      grid_auto_flow: self.grid_auto_flow.into_taffy(),
      grid_column: Line {
        start: self.grid_column_start.clone().into_taffy(),
        end: self.grid_column_end.clone().into_taffy(),
      },
      grid_row: Line {
        start: self.grid_row_start.clone().into_taffy(),
        end: self.grid_row_end.clone().into_taffy(),
      },
      grid_template_columns,
      grid_template_rows,
      grid_template_column_names,
      grid_template_row_names,
      grid_template_areas: self
        .grid_template_areas
        .as_ref()
        .cloned()
        .unwrap_or_default()
        .into_taffy(),
      aspect_ratio: self.aspect_ratio.into(),
      align_self: self.align_self.into_taffy(),
      justify_self: self.justify_self.into_taffy(),
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
