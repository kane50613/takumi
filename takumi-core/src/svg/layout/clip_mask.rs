//! Clip paths and masks, after Blink's `LayoutSVGResourceClipper` and `LayoutSVGResourceMasker`.

use std::rc::Rc;

use super::{
  Scope, SvgLayout,
  object::{Container, LayoutClip, LayoutMask, LayoutObject, Units},
};
use crate::{
  geometry::Rect,
  style::{Affine, BasicShape, Length, MaskType, Transforms},
  svg::{
    element::SvgElementKind,
    length::{LengthAxis, SvgLengthContext},
  },
};

impl SvgLayout<'_> {
  /// The clip path the element at `link` makes for content with `object_bounding_box`, none when
  /// the clip path is invalid, which hides the content.
  pub(super) fn convert_clip_path(
    &mut self,
    link: usize,
    scope: &Scope,
    object_bounding_box: Option<Rect<f32>>,
  ) -> Option<Rc<LayoutClip>> {
    if self.active.contains(&link) {
      return None;
    }

    self.active.push(link);

    let clip = self
      .deeper(|layout| layout.build_clip_path(link, scope, object_bounding_box))
      .flatten();

    self.active.pop();
    clip
  }

  fn build_clip_path(
    &mut self,
    link: usize,
    scope: &Scope,
    object_bounding_box: Option<Rect<f32>>,
  ) -> Option<Rc<LayoutClip>> {
    // A `clip-path` property must name a `clipPath` element.
    if self.document.elements[link].kind != SvgElementKind::ClipPath {
      return None;
    }

    let styled = self.styled_in_place(link);

    // The whole clip path is ignored when its transform is invalid.
    if self
      .attribute(link, "transform")
      .is_some_and(|text| Transforms::from_svg_attribute(text).is_none())
      || self
        .authored_transform(&styled, scope)
        .is_some_and(|transform| !transform.is_renderable())
    {
      return None;
    }

    let mut transform = self.resolve_transform(&styled, scope);
    let units = match self.attribute(link, "clipPathUnits") {
      Some("objectBoundingBox") => Units::ObjectBoundingBox,
      _ => Units::UserSpaceOnUse,
    };

    // Only a clip path in user units can be shared, as one in bounding box units becomes
    // specific to the content.
    let cacheable = units == Units::UserSpaceOnUse;

    if cacheable && let Some(clip) = self.clip_paths.get(&link) {
      return Some(Rc::clone(clip));
    }

    if units == Units::ObjectBoundingBox {
      transform *= object_bounding_box?.unit_square_transform();
    }

    // A linked clip path must be valid.
    let nested = match &styled.style().misc1_data.clip_path {
      Some(BasicShape::Url(resource)) => match self.reference(resource) {
        Some(nested) if !self.active.contains(&nested) => {
          Some(self.convert_clip_path(nested, scope, object_bounding_box)?)
        }
        _ => None,
      },
      _ => None,
    };
    let mut clip_scope = scope.clone();

    clip_scope.parent_clip_path = Some(link);

    let mut root = Container::new(Affine::IDENTITY);

    self.convert_clip_path_children(link, &styled.context, &clip_scope, &mut root);

    // A clip path without children is invalid.
    if !root.has_children() {
      return None;
    }

    root.finish();

    let clip = Rc::new(LayoutClip {
      transform,
      clip_path: nested,
      root,
    });

    if cacheable {
      self.clip_paths.insert(link, Rc::clone(&clip));
    }

    Some(clip)
  }

  /// The mask the element at `link` makes for content with `object_bounding_box`, none when the
  /// mask is invalid, which hides the content.
  pub(super) fn convert_mask(
    &mut self,
    link: usize,
    scope: &Scope,
    object_bounding_box: Option<Rect<f32>>,
  ) -> Option<Rc<LayoutMask>> {
    if self.active.contains(&link) {
      return None;
    }

    self.active.push(link);

    let mask = self
      .deeper(|layout| layout.build_mask(link, scope, object_bounding_box))
      .flatten();

    self.active.pop();
    mask
  }

  fn build_mask(
    &mut self,
    link: usize,
    scope: &Scope,
    object_bounding_box: Option<Rect<f32>>,
  ) -> Option<Rc<LayoutMask>> {
    // A `mask` property must name a `mask` element.
    if self.document.elements[link].kind != SvgElementKind::Mask {
      return None;
    }

    let styled = self.styled_in_place(link);
    let units = match self.attribute(link, "maskUnits") {
      Some("userSpaceOnUse") => Units::UserSpaceOnUse,
      _ => Units::ObjectBoundingBox,
    };
    let content_units = match self.attribute(link, "maskContentUnits") {
      Some("objectBoundingBox") => Units::ObjectBoundingBox,
      _ => Units::UserSpaceOnUse,
    };

    // Only a mask in user units can be shared, as one in bounding box units becomes specific to
    // the content.
    let cacheable = units == Units::UserSpaceOnUse && content_units == Units::UserSpaceOnUse;

    if cacheable && let Some(mask) = self.masks.get(&link) {
      return Some(Rc::clone(mask));
    }

    let coordinate = |layout: &Self, name: &str, axis: LengthAxis, default: f32| {
      let length = layout
        .length_attribute(link, name)
        .unwrap_or(Length::Percentage(default));

      if units == Units::ObjectBoundingBox {
        SvgLengthContext::resolve_in_bounding_box(length, styled.sizing())
      } else {
        layout.px(&styled, scope, length, axis)
      }
    };
    let mut rect = Rect::positive_from_xywh(
      coordinate(self, "x", LengthAxis::X, -10.0),
      coordinate(self, "y", LengthAxis::Y, -10.0),
      coordinate(self, "width", LengthAxis::X, 120.0),
      coordinate(self, "height", LengthAxis::Y, 120.0),
    )?;
    let mut mask_all = false;

    if units == Units::ObjectBoundingBox {
      match object_bounding_box {
        Some(bounds) => rect = rect.in_bounding_box(&bounds)?,
        // A zero-sized bounding box masks the whole element, as Chrome and Firefox do.
        None => mask_all = true,
      }
    }

    if mask_all {
      let mask = Rc::new(LayoutMask {
        rect,
        luminance: true,
        mask: None,
        root: Container::new(Affine::IDENTITY),
      });

      self.masks.insert(link, Rc::clone(&mask));
      return Some(mask);
    }

    // A linked mask must be valid.
    let nested = match self.mask_reference(styled.style()) {
      Some(nested) if !self.active.contains(&nested) => {
        Some(self.convert_mask(nested, scope, object_bounding_box)?)
      }
      _ => None,
    };
    let mut root = Container::new(Affine::IDENTITY);
    let mut sub_root = None;

    // Content in bounding box units goes under a group that scales it.
    if content_units == Units::ObjectBoundingBox {
      let transform = object_bounding_box?.unit_square_transform();
      let mut group = Container::new(transform);

      group.transform = transform;
      sub_root = Some(group);
    }

    {
      let real_root = sub_root.as_mut().unwrap_or(&mut root);

      self.convert_children(link, &styled.context, scope, real_root);

      // A mask without content is invalid.
      if !real_root.has_children() {
        return None;
      }
    }

    if let Some(mut group) = sub_root {
      group.finish();
      root.children.push(LayoutObject::Container(Box::new(group)));
    }

    root.finish();

    let mask = Rc::new(LayoutMask {
      rect,
      luminance: styled.style().svg_data.mask_type != MaskType::Alpha,
      mask: nested,
      root,
    });

    if cacheable {
      self.masks.insert(link, Rc::clone(&mask));
    }

    Some(mask)
  }
}
