// Copyright 2017 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/*!
[resvg](https://github.com/linebender/resvg) is an SVG rendering library.

Vendored from resvg 0.47.0 (Apache-2.0 OR MIT), stripped of the `text`,
`svgz`, `system-fonts`, `memmap-fonts` and `writer` features and the CLI.
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
pub mod vector;

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
  let target_size = tiny_skia::IntSize::from_wh(pixmap.width(), pixmap.height()).unwrap();
  let max_bbox = tiny_skia::IntRect::from_xywh(
    -(target_size.width() as i32) * 2,
    -(target_size.height() as i32) * 2,
    target_size.width() * 5,
    target_size.height() * 5,
  )
  .unwrap();

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

  let target_size = tiny_skia::IntSize::from_wh(pixmap.width(), pixmap.height()).unwrap();
  let max_bbox = tiny_skia::IntRect::from_xywh(
    -(target_size.width() as i32) * 2,
    -(target_size.height() as i32) * 2,
    target_size.width() * 5,
    target_size.height() * 5,
  )
  .unwrap();

  transform = transform.pre_translate(-bbox.x(), -bbox.y());

  let ctx = render::Context { max_bbox };
  render::render_node(node, &ctx, transform, pixmap);

  Some(())
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
