// Copyright 2017 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/*!
[resvg](https://github.com/linebender/resvg) is an SVG rendering library.

Vendored from resvg 0.48.1 (Apache-2.0 OR MIT), stripped of the `svgz`,
`system-fonts`, `memmap-fonts` and `writer` features and the CLI.

Text support diverges from upstream: the `fontdb` crate is replaced by a
fontique-backed face store (`usvg::text::fontdb`), `unicode-script`/
`unicode-vo` by `icu_properties`, and color glyph formats (COLR, SVG-in-font,
bitmap) are stripped — glyphs render from outlines only.
*/

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::identity_op)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::wrong_self_convention)]

use std::sync::Arc;

use usvg::filter::Filter;

pub mod usvg;

mod clip;
mod filter;
mod geom;
mod image;
mod mask;
mod path;
mod render;

/// Renders a tree onto the pixmap.
///
/// `transform` will be used as a root transform.
/// Can be used to position SVG inside the `pixmap`.
///
/// The produced content is in the sRGB color space.
pub fn render(
  tree: &crate::resvg::usvg::Tree,
  transform: tiny_skia::Transform,
  pixmap: &mut tiny_skia::PixmapMut,
) {
  let max_bbox = max_filter_bbox(pixmap.width(), pixmap.height());

  let ctx = render::Context { max_bbox };
  render::render_nodes(tree.root(), &ctx, transform, pixmap);
}

/// Renders a node onto the pixmap.
///
/// `transform` will be used as a root transform.
/// Can be used to position SVG inside the `pixmap`.
///
/// The expected pixmap size can be retrieved from `crate::resvg::usvg::Node::abs_layer_bounding_box()`.
///
/// Returns `None` when `node` has a zero size.
///
/// The produced content is in the sRGB color space.
pub fn render_node(
  node: &crate::resvg::usvg::Node,
  mut transform: tiny_skia::Transform,
  pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
  let bbox = node.abs_layer_bounding_box()?;

  let max_bbox = max_filter_bbox(pixmap.width(), pixmap.height());

  transform = transform.pre_translate(-bbox.x(), -bbox.y());

  let ctx = render::Context { max_bbox };
  render::render_node(node, &ctx, transform, pixmap);

  Some(())
}

fn max_filter_bbox(width: u32, height: u32) -> tiny_skia::IntRect {
  tiny_skia::IntRect::from_xywh(
    i32::try_from(width).unwrap_or(i32::MAX).saturating_mul(-2),
    i32::try_from(height).unwrap_or(i32::MAX).saturating_mul(-2),
    width.saturating_mul(5),
    height.saturating_mul(5),
  )
  .unwrap_or_else(|| {
    tiny_skia::IntRect::from_ltrb(i32::MIN / 2, i32::MIN / 2, i32::MAX / 2, i32::MAX / 2).unwrap()
  })
}

/// Applies parsed filters to a premultiplied RGBA layer in place.
///
/// Mirrors the `render_group` filter path: each filter runs on a pixmap sized
/// to its region (the contract the filter primitives assert), with the layer
/// placed at the region offset and the region window copied back afterwards.
///
/// Returns `None` when a pixmap cannot be allocated.
pub(crate) fn apply_filters_to_layer(
  filters: &[Arc<Filter>],
  layer: &mut [u8],
  width: u32,
  height: u32,
) -> Option<()> {
  for f in filters {
    let region = f.rect();
    let ts = tiny_skia::Transform::from_translate(-region.x(), -region.y());
    let region_int = region.transform(ts)?.to_int_rect();
    let mut sub = tiny_skia::Pixmap::new(region_int.width(), region_int.height())?;

    let dx = (-region.x()).round() as i32;
    let dy = (-region.y()).round() as i32;
    let layer_ref = tiny_skia::PixmapRef::from_bytes(layer, width, height)?;
    sub.draw_pixmap(
      dx,
      dy,
      layer_ref,
      &tiny_skia::PixmapPaint::default(),
      tiny_skia::Transform::identity(),
      None,
    );

    filter::apply(f, ts, &mut sub);

    let mut out = tiny_skia::Pixmap::new(width, height)?;
    out.draw_pixmap(
      -dx,
      -dy,
      sub.as_ref(),
      &tiny_skia::PixmapPaint::default(),
      tiny_skia::Transform::identity(),
      None,
    );
    layer.copy_from_slice(out.data());
  }

  Some(())
}

pub(crate) trait OptionLog {
  fn log_none<F: FnOnce()>(self, f: F) -> Self;
}

impl<T> OptionLog for Option<T> {
  #[inline]
  fn log_none<F: FnOnce()>(self, f: F) -> Self {
    self.or_else(|| {
      f();
      None
    })
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn max_filter_bbox_is_clamped() {
    let bbox = super::max_filter_bbox(u32::MAX, u32::MAX);
    assert_eq!(bbox.left(), i32::MIN / 2);
    assert_eq!(bbox.top(), i32::MIN / 2);
    assert_eq!(bbox.right(), i32::MAX / 2);
    assert_eq!(bbox.bottom(), i32::MAX / 2);
  }
}
