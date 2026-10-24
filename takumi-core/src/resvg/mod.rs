// Copyright 2017 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/*!
[resvg](https://github.com/linebender/resvg) is an SVG rendering library.

Vendored from resvg 0.48.1 (Apache-2.0 OR MIT), stripped of the `svgz`,
`system-fonts`, `memmap-fonts` and `writer` features, the CLI and the raster renderer. The
parser and the filter pipeline remain, and a backend draws the tree from `SvgOp`s.

Text support diverges from upstream: `<text>` matches fonts through the renderer's
`FontFallbackList` and shapes through its `HarfBuzzShaper` in place of `fontdb` and
`rustybuzz`, `unicode-script`/`unicode-vo` are replaced by `icu_properties`, and glyphs resolve
through the renderer's glyph cache, which draws COLRv0 layers and bitmap glyphs but not
SVG-in-font glyphs.
*/

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::identity_op)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::wrong_self_convention)]

#[cfg(feature = "svg-raster")]
use std::sync::Arc;

#[cfg(feature = "svg-raster")]
use usvg::filter::Filter;

#[cfg(feature = "svg-raster")]
use crate::svg_vector::SvgDrawer;

/// Applies one filter to a layer, which `transform` maps the filter's user space onto.
#[cfg(feature = "svg-raster")]
pub(crate) fn apply_filter(
  filter: &Filter,
  transform: tiny_skia::Transform,
  layer: &mut tiny_skia::Pixmap,
  drawer: &dyn SvgDrawer,
) {
  filter::apply(filter, transform, layer, drawer);
}

pub mod usvg;

#[cfg(feature = "svg-raster")]
mod filter;
#[cfg(feature = "svg-raster")]
mod geom;

#[cfg(feature = "svg-raster")]
/// Applies parsed filters to a premultiplied RGBA layer in place.
///
/// Mirrors resvg's `render_group` filter path: each filter runs on a pixmap sized
/// to its region (the contract the filter primitives assert), with the layer
/// placed at the region offset and the region window copied back afterwards.
///
/// Returns `None` when a pixmap cannot be allocated.
pub(crate) fn apply_filters_to_layer(
  filters: &[Arc<Filter>],
  layer: &mut [u8],
  width: u32,
  height: u32,
  drawer: &dyn SvgDrawer,
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

    filter::apply(f, ts, &mut sub, drawer);

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
