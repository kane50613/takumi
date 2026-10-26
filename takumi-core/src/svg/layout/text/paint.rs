//! Text as the shapes and pictures it paints, after usvg's text flattening: each glyph is its
//! outline in the fill and stroke of its span, a color glyph its layers, and a bitmap glyph a
//! picture, in the order Blink paints a span: overline, underline, glyphs, then line-through.

use std::rc::Rc;

use skrifa::{FontRef, MetadataProvider};

use super::{
  Container, Fill, Stroke, SvgLayout,
  position::{DecorationLine, LaidOutSpan, LaidOutText},
  shape::PositionedGlyph,
};
use crate::{
  geometry::{PathCommand, Rect},
  resources::{
    font::FontsSnapshot,
    glyph::{ResolvedGlyph, ResolvedOutlineGlyph},
    image::{SvgImageData, SvgLineCap, SvgLineJoin},
  },
  style::{Affine, Color, ImageScalingAlgorithm},
  svg::layout::{
    budget::Budget,
    object::{LayoutObject, Paint, Picture, PictureContent, Shape},
  },
};

/// The miter limit of the outline of a synthetic bold glyph.
const BOLD_MITER_LIMIT: f32 = 4.0;

/// The objects the spans of a text paint, in paint order. Each path command is charged to the
/// budget before it is kept, so what a text may allocate is bounded as what it may paint is.
struct TextPainter<'b> {
  budget: &'b mut Budget,
  /// Whether the budget ran out, which ends the painting.
  exhausted: bool,
  objects: Vec<LayoutObject>,
  /// The box of the text, which every object reports as its bounds.
  bounds: Rect<f32>,
  abs_transform: Affine,
  fonts: FontsSnapshot,
}

impl SvgLayout<'_> {
  /// Paints the laid out text into `parent`.
  pub(super) fn paint_text(&mut self, text: &LaidOutText, parent: &mut Container) {
    let fonts = self.styles.root_context().fonts().clone();
    let mut painter = TextPainter {
      budget: &mut self.budget,
      exhausted: false,
      objects: Vec::new(),
      bounds: text.bounds,
      abs_transform: parent.abs_transform,
      fonts,
    };

    for span in &text.spans {
      painter.paint_span(span);
    }

    parent.children.append(&mut painter.objects);
  }
}

impl TextPainter<'_> {
  fn paint_span(&mut self, span: &LaidOutSpan) {
    if self.exhausted || span.opacity <= 0.0 {
      return;
    }

    let first = self.objects.len();

    self.paint_span_objects(span);

    if span.opacity < 1.0 && self.objects.len() > first {
      let mut group = Container::new(self.abs_transform);

      group.opacity = span.opacity;
      group.children = self.objects.split_off(first);
      group.finish();
      self.objects.push(LayoutObject::Container(Box::new(group)));
    }
  }

  /// The decorations and glyphs of `span`, in paint order.
  fn paint_span_objects(&mut self, span: &LaidOutSpan) {
    self.paint_line(span.overline.as_ref());
    self.paint_line(span.underline.as_ref());

    let mut outlines = Vec::new();

    for glyph in &span.glyphs {
      if self.exhausted {
        return;
      }

      self.paint_glyph(span, glyph, &mut outlines);
    }

    self.flush(span, &mut outlines);
    self.paint_line(span.line_through.as_ref());
  }

  fn paint_line(&mut self, line: Option<&DecorationLine>) {
    if let Some(line) = line {
      self.push(
        line.visible,
        line.fill.clone(),
        line.stroke.clone(),
        false,
        line.commands.clone(),
      );
    }
  }

  fn push(
    &mut self,
    visible: bool,
    fill: Option<Fill>,
    stroke: Option<Stroke>,
    stroke_first: bool,
    commands: Vec<PathCommand>,
  ) {
    if !self.charge(commands.len()) {
      return;
    }

    self.emit(visible, fill, stroke, stroke_first, commands);
  }

  /// Takes `commands` of the budget, false once it is spent.
  fn charge(&mut self, commands: usize) -> bool {
    self.exhausted = self.exhausted || !self.budget.spend_painted(commands);
    !self.exhausted
  }

  /// Keeps a shape of commands that have been charged already.
  fn emit(
    &mut self,
    visible: bool,
    fill: Option<Fill>,
    stroke: Option<Stroke>,
    stroke_first: bool,
    commands: Vec<PathCommand>,
  ) {
    if fill.is_none() && stroke.is_none() {
      return;
    }

    // A path with a point that is not finite draws nothing.
    let Some(outline) = PathCommand::normalize(&commands) else {
      return;
    };

    let mut shape = Shape {
      commands: outline.commands,
      bounds: self.bounds,
      visible,
      fill,
      stroke,
      stroke_first,
      anti_alias: true,
      abs_transform: self.abs_transform,
    };

    shape.settle_paints(None);
    self.objects.push(LayoutObject::Shape(Box::new(shape)));
  }

  /// Paints the plain outlines collected so far as one shape in the paint of the span.
  fn flush(&mut self, span: &LaidOutSpan, outlines: &mut Vec<PathCommand>) {
    let commands = std::mem::take(outlines);

    self.emit(
      span.visible,
      span.fill.clone(),
      span.stroke.clone(),
      span.stroke_first,
      commands,
    );
  }

  /// Paints `glyph`, collecting a plain outline into `outlines` so that consecutive ones share a
  /// shape.
  fn paint_glyph(
    &mut self,
    span: &LaidOutSpan,
    glyph: &PositionedGlyph,
    outlines: &mut Vec<PathCommand>,
  ) {
    let Some(font) = glyph.font.typeface.font_ref() else {
      return;
    };
    let Some(resolved) = self
      .fonts
      .resolve_glyph(glyph.font.face(), font.clone(), glyph.id)
    else {
      return;
    };
    let transform = glyph.transform();
    let ResolvedGlyph::Outline(outline) = resolved.as_ref() else {
      self.flush(span, outlines);
      self.paint_bitmap(span, &resolved, transform);

      return;
    };
    let path = place(outline.paths(), transform);
    let layers = span.color_layer_fills(outline, &font);
    // Synthetic bold strokes the glyph in its own fill, as the raster backend does.
    let bold = outline
      .embolden()
      .filter(|width| *width > 0.0)
      .zip(span.fill.as_ref())
      .map(|(width, fill)| Stroke {
        paint: fill.paint.clone(),
        opacity: fill.opacity,
        width,
        miter_limit: BOLD_MITER_LIMIT,
        cap: SvgLineCap::Butt,
        join: SvgLineJoin::Miter,
        dash: None,
        context: fill.context,
        user_space: None,
      });

    if layers.is_empty() && bold.is_none() {
      if self.charge(path.len()) {
        outlines.extend(path);
      }

      return;
    }

    self.flush(span, outlines);

    let stroke_first = span.stroke_first;

    self.push(
      span.visible,
      None,
      span.stroke.clone().filter(|_| stroke_first),
      stroke_first,
      path.clone(),
    );

    for (fill, layer) in layers {
      self.push(
        span.visible,
        Some(fill),
        None,
        false,
        place(&layer, transform),
      );
    }

    let fill = span.fill.clone().filter(|_| bold.is_some());

    self.push(span.visible, fill, bold, stroke_first, path.clone());
    self.push(
      span.visible,
      None,
      span.stroke.clone().filter(|_| !stroke_first),
      stroke_first,
      path,
    );
  }

  /// Paints the bitmap `glyph` placed by `transform` in the fill opacity of the span, as Skia
  /// draws a color glyph in its own colors and the alpha of the paint. A span without a fill
  /// paints none.
  fn paint_bitmap(&mut self, span: &LaidOutSpan, glyph: &ResolvedGlyph, transform: Affine) {
    let ResolvedGlyph::Bitmap(bitmap) = glyph else {
      return;
    };
    let Some(fill) = &span.fill else {
      return;
    };

    // Each glyph keeps its own copy of the pixels, so they are charged as decoded image bytes.
    if !self.charge(1) || !self.budget.spend_image_bytes(bitmap.image.data().len()) {
      self.exhausted = true;
      return;
    }

    let placement = transform * bitmap.image_transform();
    let mut group = Container::new(self.abs_transform * placement);

    group.transform = placement;
    group.opacity = fill.opacity;
    group.children.push(LayoutObject::Picture(Box::new(Picture {
      content: PictureContent::Bitmap(Some(Rc::new(SvgImageData::Bitmap(bitmap.image.clone())))),
      width: bitmap.image.width() as f32,
      height: bitmap.image.height() as f32,
      visible: span.visible,
      rendering: ImageScalingAlgorithm::Auto,
    })));
    group.finish();
    self.objects.push(LayoutObject::Container(Box::new(group)));
  }
}

impl LaidOutSpan {
  /// The fills of a COLR glyph's layers over the fill of the span, as the raster backend paints
  /// them, with their outlines. Empty for a span without a fill, which paints the outline of the
  /// glyph instead.
  fn color_layer_fills(
    &self,
    outline: &ResolvedOutlineGlyph,
    font: &FontRef<'_>,
  ) -> Vec<(Fill, Vec<PathCommand>)> {
    let (Some(layers), Some(fill)) = (outline.color_layers(), self.fill.as_ref()) else {
      return Vec::new();
    };
    let palettes = font.color_palettes();
    let palette = palettes.get(0);
    let opacity = (fill.opacity * 255.0).round() as u8;
    let foreground = match &fill.paint {
      Paint::Color(Color([red, green, blue, _])) => Color([*red, *green, *blue, opacity]),
      _ => Color([0, 0, 0, opacity]),
    };

    layers
      .iter()
      .filter_map(|layer| {
        let layer_fill = if layer.is_foreground() {
          Fill {
            opacity: (fill.opacity * layer.alpha).clamp(0.0, 1.0),
            ..fill.clone()
          }
        } else {
          let Color([red, green, blue, alpha]) = layer.color(palette.as_ref(), foreground)?;

          Fill {
            paint: Paint::Color(Color([red, green, blue, 255])),
            opacity: f32::from(alpha) / 255.0,
            evenodd: false,
            context: None,
          }
        };

        Some((layer_fill, layer.paths.clone()))
      })
      .collect()
  }
}

/// `commands` under `transform`.
fn place(commands: &[PathCommand], transform: Affine) -> Vec<PathCommand> {
  commands
    .iter()
    .map(|command| command.transformed(transform))
    .collect()
}
