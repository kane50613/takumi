//! A paint device that records what the shared painters draw as drawables.

use std::ptr;

#[cfg(feature = "png")]
use super::document::{ImageSource, Sampling};
use super::{
  document::{
    Drawable, FillRuleName, LineCapName, LineJoinName, Paint, PaintPoint, PaintRect, Role,
    ShadowSide, Shape, Stroke,
  },
  runs::GlyphPaint,
};
#[cfg(feature = "png")]
use crate::resources::image::to_data_url;
use crate::{
  font_style::SizedFontStyle,
  layout::inline::PositionedInlineRun,
  painter::{
    BoxFrame, FillShape, GlyphDevice, GlyphFill, PaintDevice, PaintRole, ShadowShape, StrokeStyle,
  },
  path_data::path_data,
  shadow::SizedShadow,
  style::{Affine, Color, LineJoin},
};

/// A clip the painters pushed.
struct Clip {
  /// The region it keeps, as a shape to clip to.
  shape: Shape,
  /// The shape it keeps the inside or outside of.
  region: Shape,
  /// Whether it keeps the outside of `region`.
  outside: bool,
}

/// The runs a text node lays out, and what `background-clip: text` shows through them.
pub(super) struct RecordedText<'r> {
  /// The runs, indexed as the node's `runs` are.
  pub(super) runs: &'r [PositionedInlineRun],
  /// The box's background layers, bottom first, in the node's space.
  pub(super) background: Vec<Paint>,
}

/// Records draws in a node's local space as [`Drawable`]s.
///
/// Approximate: a layer the painters open multiplies its opacity into each draw inside it, so
/// draws that overlap inside one layer, such as the dashes of a translucent outline meeting at a
/// corner, paint darker where a real layer would not.
pub(super) struct Recorder<'r> {
  drawables: Vec<Drawable>,
  role: Role,
  clips: Vec<Clip>,
  opacities: Vec<f32>,
  shadow: Option<SizedShadow>,
  text: Option<RecordedText<'r>>,
}

impl<'r> Recorder<'r> {
  /// A recorder for a box or image.
  pub(super) fn new() -> Self {
    Self {
      drawables: Vec::new(),
      role: Role::Background,
      clips: Vec::new(),
      opacities: Vec::new(),
      shadow: None,
      text: None,
    }
  }

  /// A recorder for a text node laying out `text`.
  pub(super) fn text(text: RecordedText<'r>) -> Self {
    Self {
      text: Some(text),
      ..Self::new()
    }
  }

  /// The recorded drawables, bottom first.
  pub(super) fn finish(self) -> Vec<Drawable> {
    self.drawables
  }

  /// Records `drawable` directly.
  pub(super) fn push(&mut self, drawable: Drawable) {
    self.drawables.push(drawable);
  }

  /// `color` at the opacity of the open layers, or `None` when it shows nothing.
  fn visible(&self, color: Color) -> Option<[u8; 4]> {
    let opacity: f32 = self.opacities.iter().product();
    let [r, g, b, a] = color.0;
    let alpha = (f32::from(a) * opacity).round() as u8;

    (alpha > 0).then_some([r, g, b, alpha])
  }

  /// The open clips, as shapes to clip to at once.
  fn clips(&self) -> Vec<Shape> {
    self.clips.iter().map(|clip| clip.shape.clone()).collect()
  }

  /// The side of the innermost clip a shadow shows on, or everywhere when nothing clips it.
  fn shadow_side(&self) -> (ShadowSide, Shape) {
    match self.clips.last() {
      Some(clip) => (
        if clip.outside {
          ShadowSide::Outside
        } else {
          ShadowSide::Inside
        },
        clip.region.clone(),
      ),
      None => (
        ShadowSide::Outside,
        Shape::Rect {
          rect: PaintRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
          },
        },
      ),
    }
  }

  /// Records `shape`, filled or stroked, cast as `shadow`'s blurred shadow.
  fn cast(&mut self, shape: Shape, stroke: Option<Stroke>, shadow: &SizedShadow) {
    let Some(color) = self.visible(shadow.color) else {
      return;
    };
    let (visible, region) = self.shadow_side();

    self.drawables.push(Drawable::Shadow {
      role: self.role,
      shape,
      stroke,
      offset: shadow_offset(shadow),
      blur: shadow.blur_radius / 2.0,
      color,
      visible,
      region,
    });
  }

  /// Records the glyphs of the run at `run`, filled with `paint` and moved by `offset`.
  fn glyphs(
    &mut self,
    role: Role,
    run: usize,
    paint: Paint,
    offset: PaintPoint,
    blur: f32,
    stroke: Option<Stroke>,
  ) {
    self.drawables.push(Drawable::Glyphs {
      role,
      run,
      paint,
      offset,
      blur,
      stroke,
    });
  }
}

impl From<PaintRole> for Role {
  fn from(role: PaintRole) -> Self {
    match role {
      PaintRole::Background => Self::Background,
      PaintRole::Border => Self::Border,
      PaintRole::BoxShadow => Self::BoxShadow,
      PaintRole::Outline => Self::Outline,
      PaintRole::Image => Self::Image,
      PaintRole::Text => Self::Text,
      PaintRole::TextShadow => Self::TextShadow,
      PaintRole::TextDecoration => Self::TextDecoration,
      PaintRole::InlineBackground => Self::InlineBackground,
    }
  }
}

impl From<LineJoin> for LineJoinName {
  fn from(join: LineJoin) -> Self {
    match join {
      LineJoin::Miter => Self::Miter,
      LineJoin::Round => Self::Round,
      LineJoin::Bevel => Self::Bevel,
    }
  }
}

impl Stroke {
  /// An outline stroke of `width` joined as `join`, as text strokes and faux bold draw.
  fn outline(width: f32, join: LineJoin) -> Self {
    Self {
      width,
      dash: None,
      cap: LineCapName::Butt,
      join: join.into(),
    }
  }
}

fn shadow_offset(shadow: &SizedShadow) -> PaintPoint {
  PaintPoint {
    x: shadow.offset_x,
    y: shadow.offset_y,
  }
}

impl PaintDevice for Recorder<'_> {
  fn set_role(&mut self, role: PaintRole) {
    self.role = role.into();
  }

  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine) {
    let shape = Shape::of(shape, transform);

    if let Some(shadow) = self.shadow {
      return self.cast(shape, None, &shadow);
    }

    let Some(color) = self.visible(color) else {
      return;
    };

    self.drawables.push(Drawable::Fill {
      role: self.role,
      shape,
      paint: Paint::Color { color },
      blend_mode: None,
      clips: self.clips(),
    });
  }

  fn stroke_shape(&mut self, shape: &FillShape, stroke: &StrokeStyle, transform: Affine) {
    let shape = Shape::of(shape, transform);
    let style = Stroke {
      width: stroke.width,
      dash: stroke.dash.map(Vec::from),
      cap: if stroke.round_cap {
        LineCapName::Round
      } else {
        LineCapName::Butt
      },
      join: LineJoinName::Miter,
    };

    if let Some(shadow) = self.shadow {
      return self.cast(shape, Some(style), &shadow);
    }

    let Some(color) = self.visible(stroke.color) else {
      return;
    };

    self.drawables.push(Drawable::Stroke {
      role: self.role,
      shape,
      stroke: style,
      paint: Paint::Color { color },
      clips: self.clips(),
    });
  }

  fn push_clip(&mut self, shape: &FillShape, transform: Affine) {
    let region = Shape::of(shape, transform);

    self.clips.push(Clip {
      shape: region.clone(),
      region,
      outside: false,
    });
  }

  fn push_clip_out(&mut self, shape: &FillShape, transform: Affine) {
    self.clips.push(Clip {
      shape: Shape::outside(shape, transform),
      region: Shape::of(shape, transform),
      outside: true,
    });
  }

  fn pop_clip(&mut self) {
    self.clips.pop();
  }

  fn begin_layer(&mut self, opacity: f32) {
    self.opacities.push(opacity);
  }

  fn end_layer(&mut self) {
    self.opacities.pop();
  }

  fn fill_shadow(&mut self, shape: &ShadowShape, shadow: &SizedShadow, transform: Affine) {
    self.cast(Shape::of(&shape.fill_shape(), transform), None, shadow);
  }
}

impl GlyphDevice for Recorder<'_> {
  fn begin_shadow(&mut self, shadow: &SizedShadow) {
    self.shadow = Some(*shadow);
  }

  fn end_shadow(&mut self) {
    self.shadow = None;
  }

  fn draw_glyph_run(
    &mut self,
    run: &PositionedInlineRun,
    style: &SizedFontStyle,
    fill: GlyphFill,
    frame: BoxFrame,
  ) {
    let Some((index, background)) = self.text.as_ref().and_then(|text| {
      let index = text
        .runs
        .iter()
        .position(|candidate| ptr::eq(candidate, run))?;
      let background = match fill {
        GlyphFill::Background => text.background.clone(),
        GlyphFill::Text => Vec::new(),
      };

      Some((index, background))
    }) else {
      return;
    };
    let brush = &run.glyph_run.brush;
    let join = style.parent.stroke_linejoin;
    let text_stroke = (brush.stroke_width > 0.0)
      .then(|| Stroke::outline(brush.stroke_width, join))
      .zip(self.visible(brush.stroke_color));

    if let Some(shadow) = self.shadow {
      let Some(color) = self.visible(shadow.color) else {
        return;
      };
      let offset = shadow_offset(&shadow);
      let blur = shadow.blur_radius / 2.0;

      self.glyphs(
        Role::TextShadow,
        index,
        Paint::Color { color },
        offset,
        blur,
        None,
      );
      if let Some((stroke, _)) = text_stroke {
        self.glyphs(
          Role::TextShadow,
          index,
          Paint::Color { color },
          offset,
          blur,
          Some(stroke),
        );
      }
      return;
    }

    let origin = PaintPoint { x: 0.0, y: 0.0 };

    for paint in background {
      self.glyphs(Role::Background, index, paint.clone(), origin, 0.0, None);
      if let Some((stroke, _)) = &text_stroke {
        self.glyphs(
          Role::Background,
          index,
          paint,
          origin,
          0.0,
          Some(stroke.clone()),
        );
      }
    }

    if let Some(color) = self.visible(brush.color) {
      self.glyphs(self.role, index, Paint::Color { color }, origin, 0.0, None);
      if let Some(embolden) = run.embolden(frame.layout) {
        self.glyphs(
          self.role,
          index,
          Paint::Color { color },
          origin,
          0.0,
          Some(Stroke::outline(embolden, join)),
        );
      }
    }

    for glyph in run.placed_glyphs(frame.layout) {
      match glyph.paint {
        GlyphPaint::Outline { .. } => {}
        GlyphPaint::Layers(layers) => {
          for (color, paths) in layers {
            let Some(color) = self.visible(color) else {
              continue;
            };

            self.drawables.push(Drawable::Fill {
              role: self.role,
              shape: Shape::Path {
                d: path_data(paths, frame.place(glyph.transform)),
                fill_rule: FillRuleName::Nonzero,
              },
              paint: Paint::Color { color },
              blend_mode: None,
              clips: Vec::new(),
            });
          }
        }
        #[cfg(feature = "png")]
        GlyphPaint::Bitmap(bitmap) => {
          let Some(png) = bitmap.image.encode_png() else {
            continue;
          };
          let [width, height] = [bitmap.image.width(), bitmap.image.height()].map(|n| n as f32);
          let placed = frame.place(glyph.transform)
            * Affine::translation(bitmap.placement.left as f32, -(bitmap.placement.top as f32))
            * Affine::scale(bitmap.scale_x, bitmap.scale_y);
          let rect = PaintRect::bounding(width, height, placed);

          self.drawables.push(Drawable::Image {
            role: self.role,
            image: ImageSource {
              src: to_data_url("image/png", &png),
              width,
              height,
            },
            rect,
            clip: Shape::Rect { rect },
            sampling: Sampling::Smooth,
          });
        }
        // Approximate: without an encoder for the bitmap, it draws nothing.
        #[cfg(not(feature = "png"))]
        GlyphPaint::Bitmap(_) => {}
      }
    }

    if let Some((stroke, color)) = text_stroke {
      self.glyphs(
        Role::TextStroke,
        index,
        Paint::Color { color },
        origin,
        0.0,
        Some(stroke),
      );
    }
  }
}
