// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::resvg::OptionLog;

pub struct Context {
  pub max_bbox: tiny_skia::IntRect,
}

pub fn render_nodes(
  parent: &crate::resvg::usvg::Group,
  ctx: &Context,
  transform: tiny_skia::Transform,
  pixmap: &mut tiny_skia::PixmapMut,
) {
  for node in parent.children() {
    render_node(node, ctx, transform, pixmap);
  }
}

pub fn render_node(
  node: &crate::resvg::usvg::Node,
  ctx: &Context,
  transform: tiny_skia::Transform,
  pixmap: &mut tiny_skia::PixmapMut,
) {
  match node {
    crate::resvg::usvg::Node::Group(group) => {
      render_group(group, ctx, transform, pixmap);
    }
    crate::resvg::usvg::Node::Path(path) => {
      crate::resvg::path::render(
        path,
        tiny_skia::BlendMode::SourceOver,
        ctx,
        transform,
        pixmap,
      );
    }
    crate::resvg::usvg::Node::Image(image) => {
      crate::resvg::image::render(image, transform, pixmap);
    }
    crate::resvg::usvg::Node::Text(text) => {
      render_group(text.flattened(), ctx, transform, pixmap);
    }
  }
}

fn render_group(
  group: &crate::resvg::usvg::Group,
  ctx: &Context,
  transform: tiny_skia::Transform,
  pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
  let transform = transform.pre_concat(group.transform());

  if !group.should_isolate() {
    render_nodes(group, ctx, transform, pixmap);
    return Some(());
  }

  let bbox = group.layer_bounding_box().transform(transform)?;

  let mut ibbox = if group.filters().is_empty() {
    // Convert group bbox into an integer one, expanding each side outwards by 2px
    // to make sure that anti-aliased pixels would not be clipped.
    tiny_skia::IntRect::from_xywh(
      (bbox.x().floor() as i32).checked_sub(2)?,
      (bbox.y().floor() as i32).checked_sub(2)?,
      (bbox.width().ceil() as u32).checked_add(4)?,
      (bbox.height().ceil() as u32).checked_add(4)?,
    )?
  } else {
    // The bounding box for groups with filters is special and should not be expanded by 2px,
    // because it's already acting as a clipping region.
    let bbox = tiny_skia::IntRect::from_xywh(
      bbox.x().floor() as i32,
      bbox.y().floor() as i32,
      bbox.width().ceil().max(1.0) as u32,
      bbox.height().ceil().max(1.0) as u32,
    )?;
    // Make sure our filter region is not bigger than 4x the canvas size.
    // This is required mainly to prevent huge filter regions that would tank the performance.
    // It should not affect the final result in any way.
    crate::resvg::geom::fit_to_rect(bbox, ctx.max_bbox)?
  };

  // Make sure our layer is not bigger than 4x the canvas size.
  // This is required to prevent huge layers.
  if group.filters().is_empty() {
    ibbox = crate::resvg::geom::fit_to_rect(ibbox, ctx.max_bbox)?;
  }

  let shift_ts = {
    // Original shift.
    let mut dx = bbox.x();
    let mut dy = bbox.y();

    // Account for subpixel positioned layers.
    dx -= bbox.x() - ibbox.x() as f32;
    dy -= bbox.y() - ibbox.y() as f32;

    tiny_skia::Transform::from_translate(-dx, -dy)
  };

  let transform = shift_ts.pre_concat(transform);

  let mut sub_pixmap = tiny_skia::Pixmap::new(ibbox.width(), ibbox.height())
    .log_none(|| log::warn!("Failed to allocate a group layer for: {:?}.", ibbox))?;

  render_nodes(group, ctx, transform, &mut sub_pixmap.as_mut());

  if !group.filters().is_empty() {
    for filter in group.filters() {
      crate::resvg::filter::apply(filter, transform, &mut sub_pixmap);
    }
  }

  if let Some(clip_path) = group.clip_path() {
    crate::resvg::clip::apply(clip_path, transform, &mut sub_pixmap);
  }

  if let Some(mask) = group.mask() {
    crate::resvg::mask::apply(mask, ctx, transform, &mut sub_pixmap);
  }

  let paint = tiny_skia::PixmapPaint {
    opacity: group.opacity().get(),
    blend_mode: convert_blend_mode(group.blend_mode()),
    quality: tiny_skia::FilterQuality::Nearest,
  };

  pixmap.draw_pixmap(
    ibbox.x(),
    ibbox.y(),
    sub_pixmap.as_ref(),
    &paint,
    tiny_skia::Transform::identity(),
    None,
  );

  Some(())
}

pub fn convert_blend_mode(mode: crate::resvg::usvg::BlendMode) -> tiny_skia::BlendMode {
  match mode {
    crate::resvg::usvg::BlendMode::Normal => tiny_skia::BlendMode::SourceOver,
    crate::resvg::usvg::BlendMode::Multiply => tiny_skia::BlendMode::Multiply,
    crate::resvg::usvg::BlendMode::Screen => tiny_skia::BlendMode::Screen,
    crate::resvg::usvg::BlendMode::Overlay => tiny_skia::BlendMode::Overlay,
    crate::resvg::usvg::BlendMode::Darken => tiny_skia::BlendMode::Darken,
    crate::resvg::usvg::BlendMode::Lighten => tiny_skia::BlendMode::Lighten,
    crate::resvg::usvg::BlendMode::ColorDodge => tiny_skia::BlendMode::ColorDodge,
    crate::resvg::usvg::BlendMode::ColorBurn => tiny_skia::BlendMode::ColorBurn,
    crate::resvg::usvg::BlendMode::HardLight => tiny_skia::BlendMode::HardLight,
    crate::resvg::usvg::BlendMode::SoftLight => tiny_skia::BlendMode::SoftLight,
    crate::resvg::usvg::BlendMode::Difference => tiny_skia::BlendMode::Difference,
    crate::resvg::usvg::BlendMode::Exclusion => tiny_skia::BlendMode::Exclusion,
    crate::resvg::usvg::BlendMode::Hue => tiny_skia::BlendMode::Hue,
    crate::resvg::usvg::BlendMode::Saturation => tiny_skia::BlendMode::Saturation,
    crate::resvg::usvg::BlendMode::Color => tiny_skia::BlendMode::Color,
    crate::resvg::usvg::BlendMode::Luminosity => tiny_skia::BlendMode::Luminosity,
  }
}

#[cfg(test)]
mod tests {
  use crate::resvg::usvg;

  // Derived from https://github.com/servo/servo/issues/42258.
  #[test]
  fn filter_bbox_outside_int_rect() {
    let svg = r#"<svg filter="url(#f)"><filter id="f" x="2em"><feFlood/></filter><path d="M0 0H1e8V1"/></svg>"#;
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).unwrap();
    let mut pixmap = tiny_skia::Pixmap::new(1, 1).unwrap();

    // Just make sure we don't panic.
    crate::resvg::render(
      &tree,
      tiny_skia::Transform::identity(),
      &mut pixmap.as_mut(),
    );
  }
}
