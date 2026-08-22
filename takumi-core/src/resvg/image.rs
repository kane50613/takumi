// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub fn render(
  image: &crate::resvg::usvg::Image,
  transform: tiny_skia::Transform,
  pixmap: &mut tiny_skia::PixmapMut,
) {
  if !image.is_visible() {
    return;
  }

  render_inner(image.kind(), transform, image.rendering_mode(), pixmap);
}

pub fn render_inner(
  image_kind: &crate::resvg::usvg::ImageKind,
  transform: tiny_skia::Transform,
  #[allow(unused_variables)] rendering_mode: crate::resvg::usvg::ImageRendering,
  pixmap: &mut tiny_skia::PixmapMut,
) {
  match image_kind {
    crate::resvg::usvg::ImageKind::SVG(tree) => {
      render_vector(tree, transform, pixmap);
    }
    _ => {
      raster_images::render_raster(image_kind, transform, rendering_mode, pixmap);
    }
  }
}

fn render_vector(
  tree: &crate::resvg::usvg::Tree,
  transform: tiny_skia::Transform,
  pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
  let mut sub_pixmap = tiny_skia::Pixmap::new(pixmap.width(), pixmap.height()).unwrap();
  crate::resvg::render(tree, transform, &mut sub_pixmap.as_mut());
  pixmap.draw_pixmap(
    0,
    0,
    sub_pixmap.as_ref(),
    &tiny_skia::PixmapPaint::default(),
    tiny_skia::Transform::default(),
    None,
  );

  Some(())
}

mod raster_images {
  use std::sync::Arc;

  use crate::resources::image_buffer::ImageBuffer;
  use crate::resources::image_decoder::{decode_gif_frames, decode_image};
  use crate::resvg::OptionLog;
  use crate::resvg::usvg::ImageRendering;

  fn pixmap_from_premultiplied(
    data: Vec<u8>,
    width: u32,
    height: u32,
  ) -> Option<tiny_skia::Pixmap> {
    let size = tiny_skia::IntSize::from_wh(width, height)?;
    tiny_skia::Pixmap::from_vec(data, size)
  }

  fn buffer_to_pixmap(buffer: ImageBuffer) -> Option<tiny_skia::Pixmap> {
    let (width, height) = (buffer.width(), buffer.height());
    pixmap_from_premultiplied(buffer.into_premultiplied_rgba(), width, height)
  }

  fn decode_raster(image: &crate::resvg::usvg::ImageKind) -> Option<tiny_skia::Pixmap> {
    use crate::resvg::usvg::ImageKind;

    match image {
      ImageKind::SVG(_) => None,
      ImageKind::JPEG(data) | ImageKind::PNG(data) | ImageKind::WEBP(data) => decode_image(data)
        .ok()
        .and_then(buffer_to_pixmap)
        .log_none(|| log::warn!("Failed to decode an image.")),
      ImageKind::GIF(data) => {
        let mut first = None;
        decode_gif_frames(data, 0, Some(1), None, |frame| first = Some(frame)).ok()?;
        first
          .and_then(|frame| {
            let (width, height) = (frame.width(), frame.height());
            pixmap_from_premultiplied(
              Arc::unwrap_or_clone(frame).into_premultiplied_rgba(),
              width,
              height,
            )
          })
          .log_none(|| log::warn!("Failed to decode a GIF image."))
      }
    }
  }

  pub(crate) fn render_raster(
    image: &crate::resvg::usvg::ImageKind,
    transform: tiny_skia::Transform,
    rendering_mode: crate::resvg::usvg::ImageRendering,
    pixmap: &mut tiny_skia::PixmapMut,
  ) -> Option<()> {
    let raster = decode_raster(image)?;

    let rect =
      tiny_skia::Size::from_wh(raster.width() as f32, raster.height() as f32)?.to_rect(0.0, 0.0)?;

    let quality = match rendering_mode {
      ImageRendering::OptimizeQuality => tiny_skia::FilterQuality::Bicubic,
      ImageRendering::OptimizeSpeed => tiny_skia::FilterQuality::Nearest,
      ImageRendering::Smooth => tiny_skia::FilterQuality::Bilinear,
      ImageRendering::HighQuality => tiny_skia::FilterQuality::Bicubic,
      ImageRendering::CrispEdges => tiny_skia::FilterQuality::Nearest,
      ImageRendering::Pixelated => tiny_skia::FilterQuality::Nearest,
    };

    let pattern = tiny_skia::Pattern::new(
      raster.as_ref(),
      tiny_skia::SpreadMode::Pad,
      quality,
      1.0,
      tiny_skia::Transform::default(),
    );
    let mut paint = tiny_skia::Paint::default();
    paint.shader = pattern;

    pixmap.fill_rect(rect, &paint, transform, None);

    Some(())
  }
}
