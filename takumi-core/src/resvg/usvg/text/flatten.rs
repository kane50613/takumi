// Copyright 2022 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::iter::once;
use std::mem;
use std::sync::Arc;

use skrifa::MetadataProvider;
use strict_num::NonZeroPositiveF32;
use tiny_skia_path::{NonZeroRect, Transform};

use crate::geometry::PathCommand;
use crate::resources::glyph::{ResolvedBitmapGlyph, ResolvedGlyph, ResolvedOutlineGlyph};
use crate::resvg::usvg::*;
use crate::style::Color as RgbaColor;

fn resolve_rendering_mode(text: &Text) -> ShapeRendering {
  match text.rendering_mode {
    TextRendering::OptimizeSpeed => ShapeRendering::CrispEdges,
    TextRendering::OptimizeLegibility => ShapeRendering::GeometricPrecision,
    TextRendering::GeometricPrecision => ShapeRendering::GeometricPrecision,
  }
}

/// The paths a `<text>` flattens to, in paint order.
struct TextPaths {
  children: Vec<Node>,
  rendering_mode: ShapeRendering,
  abs_transform: Transform,
}

impl TextPaths {
  fn push(
    &mut self,
    span: &layout::Span,
    fill: Option<Fill>,
    stroke: Option<Stroke>,
    path: tiny_skia_path::Path,
  ) {
    if let Some(path) = Path::new(
      String::new(),
      span.visible,
      fill,
      stroke,
      span.paint_order,
      self.rendering_mode,
      Arc::new(path),
      self.abs_transform,
    ) {
      self.children.push(Node::Path(Box::new(path)));
    }
  }

  fn push_decoration(&mut self, path: Option<&Path>) {
    if let Some(path) = path {
      let mut path = path.clone();

      path.rendering_mode = self.rendering_mode;
      self.children.push(Node::Path(Box::new(path)));
    }
  }

  /// Pushes the plain outlines `builder` collected as one path in the span's paint.
  fn flush(&mut self, span: &layout::Span, builder: &mut tiny_skia_path::PathBuilder) {
    let builder = mem::replace(builder, tiny_skia_path::PathBuilder::new());

    if let Some(path) = builder.finish() {
      self.push(span, span.fill.clone(), span.stroke.clone(), path);
    }
  }

  /// Pushes `bitmap`, placed by `transform`, in the span's fill opacity: Skia draws a color glyph
  /// in its own colors and the paint's alpha, and nothing without a fill.
  fn push_bitmap(
    &mut self,
    span: &layout::Span,
    bitmap: &ResolvedBitmapGlyph,
    transform: Transform,
  ) -> Option<()> {
    let fill = span.fill.as_ref()?;
    let size = Size::from_wh(bitmap.image.width() as f32, bitmap.image.height() as f32)?;
    let image_transform = transform.pre_concat(bitmap.image_transform().into());
    let abs_transform = self.abs_transform.pre_concat(image_transform);
    let mut group = Group::empty();

    group.children.push(Node::Image(Box::new(Image {
      id: String::new(),
      visible: span.visible,
      size,
      rendering_mode: ImageRendering::default(),
      kind: ImageKind::Bitmap(Arc::new(bitmap.image.clone())),
      abs_transform,
      abs_bounding_box: size.to_non_zero_rect(0.0, 0.0).transform(abs_transform)?,
    })));
    group.transform = image_transform;
    group.abs_transform = abs_transform;
    group.opacity = fill.opacity;
    group.calculate_bounding_boxes();
    self.children.push(Node::Group(Box::new(group)));

    Some(())
  }

  /// Pushes `glyph`, collecting a plain outline into `builder` so consecutive ones share a path.
  fn push_glyph(
    &mut self,
    span: &layout::Span,
    glyph: &layout::PositionedGlyph,
    builder: &mut tiny_skia_path::PathBuilder,
    cache: &Cache,
  ) -> Option<()> {
    let font = glyph.font.typeface.font_ref()?;
    let glyphs = cache
      .fonts
      .resolve_glyphs(glyph.font.face(), font, once(glyph.id.0));
    let transform = glyph.transform();
    let outline = match glyphs.get(&glyph.id.0)?.as_ref() {
      ResolvedGlyph::Outline(outline) => outline,
      ResolvedGlyph::Bitmap(bitmap) => {
        self.flush(span, builder);

        return self.push_bitmap(span, bitmap, transform);
      }
    };
    let path = PathCommand::build_path(outline.paths())?.transform(transform)?;
    let layers = span.color_layer_fills(outline, glyph);
    // Synthetic bold strokes the glyph in its own fill, as the raster backend does.
    let bold = outline
      .embolden()
      .and_then(NonZeroPositiveF32::new)
      .zip(span.fill.as_ref())
      .map(|(width, fill)| Stroke {
        paint: fill.paint.clone(),
        dasharray: None,
        dashoffset: 0.0,
        miterlimit: StrokeMiterlimit::default(),
        opacity: fill.opacity,
        width,
        linecap: LineCap::default(),
        linejoin: LineJoin::default(),
        context_element: fill.context_element.clone(),
      });

    if layers.is_empty() && bold.is_none() {
      builder.push_path(&path);

      return Some(());
    }

    self.flush(span, builder);

    let stroke_first = span.paint_order == PaintOrder::StrokeAndFill;

    self.push(
      span,
      None,
      span.stroke.clone().filter(|_| stroke_first),
      path.clone(),
    );

    for (fill, layer_path) in layers {
      if let Some(layer_path) = layer_path.transform(transform) {
        self.push(span, Some(fill), None, layer_path);
      }
    }

    let fill = span.fill.clone().filter(|_| bold.is_some());

    self.push(span, fill, bold, path.clone());
    self.push(
      span,
      None,
      span.stroke.clone().filter(|_| !stroke_first),
      path,
    );

    Some(())
  }
}

impl layout::Span {
  /// The fills of a COLR glyph's layers over the span's fill, as the raster backend paints them;
  /// empty for a span without a fill, which paints the glyph's outline instead.
  fn color_layer_fills(
    &self,
    outline: &ResolvedOutlineGlyph,
    glyph: &layout::PositionedGlyph,
  ) -> Vec<(Fill, tiny_skia_path::Path)> {
    let (Some(layers), Some(fill)) = (outline.color_layers(), self.fill.as_ref()) else {
      return Vec::new();
    };
    let font = glyph.font.typeface.font_ref();
    let palettes = font.as_ref().map(MetadataProvider::color_palettes);
    let palette = palettes.as_ref().and_then(|palettes| palettes.get(0));
    let opacity = (fill.opacity.get() * 255.0).round() as u8;
    let foreground = match &fill.paint {
      Paint::Color(color) => RgbaColor([color.red, color.green, color.blue, opacity]),
      _ => RgbaColor([0, 0, 0, opacity]),
    };

    layers
      .iter()
      .filter_map(|layer| {
        let path = PathCommand::build_path(&layer.paths)?;
        let layer_fill = if layer.is_foreground() {
          Fill {
            opacity: Opacity::new_clamped(fill.opacity.get() * layer.alpha),
            ..fill.clone()
          }
        } else {
          let RgbaColor([red, green, blue, alpha]) = layer.color(palette.as_ref(), foreground)?;

          Fill {
            paint: Paint::Color(Color::new_rgb(red, green, blue)),
            opacity: Opacity::new_clamped(f32::from(alpha) / 255.0),
            rule: FillRule::NonZero,
            context_element: None,
          }
        };

        Some((layer_fill, path))
      })
      .collect()
  }
}

pub(crate) fn flatten(text: &mut Text, cache: &Cache) -> Option<(Group, NonZeroRect)> {
  let mut paths = TextPaths {
    children: Vec::new(),
    rendering_mode: resolve_rendering_mode(text),
    abs_transform: text.abs_transform,
  };

  for span in &text.layouted {
    paths.push_decoration(span.overline.as_ref());
    paths.push_decoration(span.underline.as_ref());

    let mut builder = tiny_skia_path::PathBuilder::new();

    for glyph in &span.positioned_glyphs {
      paths.push_glyph(span, glyph, &mut builder, cache);
    }

    paths.flush(span, &mut builder);
    paths.push_decoration(span.line_through.as_ref());
  }

  let mut group = Group {
    id: text.id.clone(),
    children: paths.children,
    ..Group::empty()
  };

  group.calculate_bounding_boxes();
  let stroke_bbox = group.stroke_bounding_box().to_non_zero_rect()?;
  Some((group, stroke_bbox))
}
