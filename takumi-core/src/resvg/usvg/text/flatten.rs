// Copyright 2022 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::mem;
use std::sync::Arc;

use super::layout::ResolvedFont;
use crate::resvg::usvg::GlyphId;
use skrifa::MetadataProvider;
use skrifa::Tag;
use skrifa::outline::{DrawSettings, OutlinePen};
use tiny_skia_path::{NonZeroRect, Transform};

use crate::resources::glyph::draw_outline;
use crate::resvg::usvg::*;

fn resolve_rendering_mode(text: &Text) -> ShapeRendering {
  match text.rendering_mode {
    TextRendering::OptimizeSpeed => ShapeRendering::CrispEdges,
    TextRendering::OptimizeLegibility => ShapeRendering::GeometricPrecision,
    TextRendering::GeometricPrecision => ShapeRendering::GeometricPrecision,
  }
}

fn push_outline_paths(
  span: &layout::Span,
  builder: &mut tiny_skia_path::PathBuilder,
  new_children: &mut Vec<Node>,
  rendering_mode: ShapeRendering,
  abs_transform: Transform,
) {
  let builder = mem::replace(builder, tiny_skia_path::PathBuilder::new());

  if let Some(path) = builder.finish().and_then(|p| {
    Path::new(
      String::new(),
      span.visible,
      span.fill.clone(),
      span.stroke.clone(),
      span.paint_order,
      rendering_mode,
      Arc::new(p),
      abs_transform,
    )
  }) {
    new_children.push(Node::Path(Box::new(path)));
  }
}

pub(crate) fn flatten(text: &mut Text, cache: &mut Cache) -> Option<(Group, NonZeroRect)> {
  let mut new_children = vec![];

  let abs_transform = text.abs_transform;
  let rendering_mode = resolve_rendering_mode(text);

  for span in &text.layouted {
    if let Some(path) = span.overline.as_ref() {
      let mut path = path.clone();
      path.rendering_mode = rendering_mode;
      new_children.push(Node::Path(Box::new(path)));
    }

    if let Some(path) = span.underline.as_ref() {
      let mut path = path.clone();
      path.rendering_mode = rendering_mode;
      new_children.push(Node::Path(Box::new(path)));
    }

    // Instead of always processing each glyph separately, we always collect
    // as many outline glyphs as possible by pushing them into the span_builder
    // and only if we encounter a different glyph, or we reach the very end of the
    // span to we push the actual outline paths into new_children. This way, we don't need
    // to create a new path for every glyph if we have many consecutive glyphs
    // with just outlines (which is the most common case).
    let mut span_builder = tiny_skia_path::PathBuilder::new();

    for glyph in &span.positioned_glyphs {
      // Color glyph formats (COLR/SVG/bitmap) are stripped from this vendor;
      // every glyph renders from its outline table.
      let outline = cache.outline(&glyph.font, glyph.id);

      if let Some(outline) = outline.and_then(|p| p.transform(glyph.outline_transform())) {
        span_builder.push_path(&outline);
      }
    }

    push_outline_paths(
      span,
      &mut span_builder,
      &mut new_children,
      rendering_mode,
      abs_transform,
    );

    if let Some(path) = span.line_through.as_ref() {
      let mut path = path.clone();
      path.rendering_mode = rendering_mode;
      new_children.push(Node::Path(Box::new(path)));
    }
  }

  let mut group = Group {
    id: text.id.clone(),
    ..Group::empty()
  };

  for child in new_children {
    group.children.push(child);
  }

  group.calculate_bounding_boxes();
  let stroke_bbox = group.stroke_bounding_box().to_non_zero_rect()?;
  Some((group, stroke_bbox))
}

#[derive(Default)]
struct PathBuilder {
  builder: tiny_skia_path::PathBuilder,
}

impl OutlinePen for PathBuilder {
  fn move_to(&mut self, x: f32, y: f32) {
    self.builder.move_to(x, y);
  }

  fn line_to(&mut self, x: f32, y: f32) {
    self.builder.line_to(x, y);
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    self.builder.quad_to(cx0, cy0, x, y);
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    self.builder.cubic_to(cx0, cy0, cx1, cy1, x, y);
  }

  fn close(&mut self) {
    self.builder.close();
  }
}

impl ResolvedFont {
  /// The glyph's outline in font units, at the face's variation coordinates.
  pub(crate) fn outline(&self, glyph_id: GlyphId) -> Option<tiny_skia_path::Path> {
    let font = self.typeface.font_ref()?;
    let outline = font.outline_glyphs().get(glyph_id.into())?;
    let mut builder = PathBuilder::default();
    let location = font.axes().location(
      self
        .variations
        .iter()
        .map(|variation| (Tag::from_be_bytes(variation.tag), variation.value)),
    );

    draw_outline(
      &outline,
      DrawSettings::unhinted(skrifa::prelude::Size::unscaled(), &location),
      &mut builder,
    )
    .ok()?;

    builder.builder.finish()
  }
}
