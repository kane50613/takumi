//! Draws the COLR glyphs takumi-core records onto a writer surface.

use skrifa::{
  FontRef, GlyphId, MetadataProvider,
  color::{ColorPalette, CompositeMode, Extend},
  instance::LocationRef,
};
use takumi_core::path::{Path, PathBuilder};
use takumi_core::resources::color_glyph::{
  ColorGlyph, ColorInstruction, ColorPaint, ColorRef, ColorStop,
};

use crate::writer::{
  blend::BlendMode,
  color::rgb,
  geom::{Path as PdfPath, Transform},
  mask::{Mask, MaskType},
  num::NormalizedF32,
  paint::{Fill, FillRule, LinearGradient, RadialGradient, SpreadMethod, Stop, SweepGradient},
  surface::Surface,
};

/// Draws the COLR drawing of `glyph` in the text color `foreground`, or returns `None` when the
/// font has none for it. Nothing is drawn unless the whole drawing converts.
pub(crate) fn draw(
  font: &FontRef<'_>,
  location: LocationRef<'_>,
  glyph: GlyphId,
  foreground: rgb::Color,
  surface: &mut Surface,
) -> Option<()> {
  let color_glyph = ColorGlyph::record(font, location, glyph)?;
  let palettes = font.color_palettes();
  let colors = Colors {
    palette: palettes.get(0),
    foreground,
  };
  let instructions = colors.instructions(color_glyph.instructions)?;
  let Some(bounds) = color_glyph.bounds.filter(|rect| !rect.is_empty()) else {
    return Some(());
  };
  let bounds = PathBuilder::from_rect(bounds);

  surface.push_transform(&Transform::from_scale(1.0, -1.0));
  interpret(instructions, &bounds, surface);
  surface.pop();

  Some(())
}

/// A [`ColorInstruction`] with its colors resolved into writer fills.
enum Instruction {
  Layer(CompositeMode, Vec<Instruction>),
  Composite {
    source: Vec<Instruction>,
    mode: CompositeMode,
    backdrop: Vec<Instruction>,
  },
  Filled(Box<Fill>, Vec<Path>),
}

/// What a glyph's palette indices resolve to.
struct Colors<'a> {
  palette: Option<ColorPalette<'a>>,
  foreground: rgb::Color,
}

impl Colors<'_> {
  fn instructions(&self, instructions: Vec<ColorInstruction>) -> Option<Vec<Instruction>> {
    instructions
      .into_iter()
      .map(|instruction| {
        Some(match instruction {
          ColorInstruction::Layer(mode, instructions) => {
            Instruction::Layer(mode, self.instructions(instructions)?)
          }
          ColorInstruction::Composite {
            source,
            mode,
            backdrop,
          } => Instruction::Composite {
            source: self.instructions(source)?,
            mode,
            backdrop: self.instructions(backdrop)?,
          },
          ColorInstruction::Filled(paint, clips) => {
            Instruction::Filled(Box::new(self.fill(paint)?), clips)
          }
        })
      })
      .collect()
  }

  fn color(&self, color: ColorRef) -> Option<(rgb::Color, NormalizedF32)> {
    if color.is_foreground() {
      return Some((self.foreground, NormalizedF32::new(color.alpha)?));
    }

    let record = self
      .palette
      .as_ref()?
      .colors()
      .get(usize::from(color.palette_index))?;

    Some((
      rgb::Color::new(record.red, record.green, record.blue),
      NormalizedF32::new(color.alpha * f32::from(record.alpha) / 255.0)?,
    ))
  }

  fn stops(&self, stops: Vec<ColorStop>) -> Option<Vec<Stop>> {
    stops
      .into_iter()
      .map(|stop| {
        let (color, opacity) = self.color(stop.color)?;

        Some(Stop {
          offset: NormalizedF32::new(stop.offset)?,
          color: color.into(),
          opacity,
        })
      })
      .collect()
  }

  fn fill(&self, paint: ColorPaint) -> Option<Fill> {
    let (paint, opacity) = match paint {
      ColorPaint::Solid(color) => {
        let (color, opacity) = self.color(color)?;

        (color.into(), opacity)
      }
      ColorPaint::Linear {
        p0,
        p1,
        stops,
        extend,
        transform,
      } => (
        LinearGradient {
          x1: p0.x,
          y1: p0.y,
          x2: p1.x,
          y2: p1.y,
          stops: self.stops(stops)?,
          spread_method: spread_method(extend),
          transform: Transform::from_affine(transform),
          anti_alias: false,
        }
        .into(),
        NormalizedF32::ONE,
      ),
      ColorPaint::Radial {
        c0,
        r0,
        c1,
        r1,
        stops,
        extend,
        transform,
      } => (
        RadialGradient {
          fx: c0.x,
          fy: c0.y,
          fr: r0,
          cx: c1.x,
          cy: c1.y,
          cr: r1,
          stops: self.stops(stops)?,
          spread_method: spread_method(extend),
          transform: Transform::from_affine(transform),
          anti_alias: false,
        }
        .into(),
        NormalizedF32::ONE,
      ),
      // The writer's sweep gradients turn the other way than COLR's, so the y-axis flips.
      ColorPaint::Sweep {
        center,
        start_angle,
        end_angle,
        stops,
        extend,
        transform,
      } => (
        SweepGradient {
          cx: center.x,
          cy: -center.y,
          start_angle,
          end_angle,
          stops: self.stops(stops)?,
          spread_method: spread_method(extend),
          transform: Transform::from_affine(transform).pre_concat(Transform::from_scale(1.0, -1.0)),
          anti_alias: false,
        }
        .into(),
        NormalizedF32::ONE,
      ),
    };

    Some(Fill {
      paint,
      opacity,
      rule: FillRule::default(),
    })
  }
}

fn spread_method(extend: Extend) -> SpreadMethod {
  match extend {
    Extend::Repeat => SpreadMethod::Repeat,
    Extend::Reflect => SpreadMethod::Reflect,
    Extend::Pad | Extend::Unknown => SpreadMethod::Pad,
  }
}

fn blend_mode(mode: CompositeMode) -> Option<BlendMode> {
  Some(match mode {
    CompositeMode::SrcOver => BlendMode::Normal,
    CompositeMode::Screen => BlendMode::Screen,
    CompositeMode::Overlay => BlendMode::Overlay,
    CompositeMode::Darken => BlendMode::Darken,
    CompositeMode::Lighten => BlendMode::Lighten,
    CompositeMode::ColorDodge => BlendMode::ColorDodge,
    CompositeMode::ColorBurn => BlendMode::ColorBurn,
    CompositeMode::HardLight => BlendMode::HardLight,
    CompositeMode::SoftLight => BlendMode::SoftLight,
    CompositeMode::Difference => BlendMode::Difference,
    CompositeMode::Exclusion => BlendMode::Exclusion,
    CompositeMode::Multiply => BlendMode::Multiply,
    CompositeMode::HslHue => BlendMode::Hue,
    CompositeMode::HslColor => BlendMode::Color,
    CompositeMode::HslLuminosity => BlendMode::Luminosity,
    CompositeMode::HslSaturation => BlendMode::Saturation,
    _ => return None,
  })
}

fn interpret(instructions: Vec<Instruction>, bounds: &Path, surface: &mut Surface) {
  for instruction in instructions {
    match instruction {
      Instruction::Layer(mode, instructions) => {
        surface.push_blend_mode(blend_mode(mode).unwrap_or(BlendMode::Normal));
        surface.push_isolated();
        interpret(instructions, bounds, surface);
        surface.pop();
        surface.pop();
      }
      Instruction::Composite {
        source,
        mode,
        backdrop,
      } => interpret_composite(source, mode, backdrop, bounds, surface),
      Instruction::Filled(fill, mut clips) => {
        // Every clip but the last applies, and the last is the filled path, since filling the
        // clipped area whole leaves artifacts in Chrome's PDF viewer when zooming.
        let filled = clips.pop().unwrap_or_else(|| bounds.clone());

        let clips: Vec<PdfPath> = clips.into_iter().map(PdfPath).collect();
        let clip_count = clips.len();

        for path in &clips {
          surface.push_clip_path(path, &FillRule::NonZero);
        }

        let old_fill = surface.get_fill().cloned();
        let old_stroke = surface.get_stroke().cloned();

        surface.set_fill(Some(*fill));
        surface.set_stroke(None);
        surface.draw_path(&PdfPath(filled));
        surface.set_fill(old_fill);
        surface.set_stroke(old_stroke);

        for _ in 0..clip_count {
          surface.pop();
        }
      }
    }
  }
}

fn interpret_composite(
  source: Vec<Instruction>,
  mode: CompositeMode,
  backdrop: Vec<Instruction>,
  bounds: &Path,
  surface: &mut Surface,
) {
  match mode {
    CompositeMode::Clear => {}
    CompositeMode::Src => {
      surface.push_isolated();
      interpret(source, bounds, surface);
      surface.pop();
    }
    CompositeMode::Dest => {
      surface.push_isolated();
      interpret(backdrop, bounds, surface);
      surface.pop();
    }
    CompositeMode::SrcOver => {
      surface.push_isolated();
      interpret(backdrop, bounds, surface);
      interpret(source, bounds, surface);
      surface.pop();
    }
    CompositeMode::DestOver => {
      surface.push_isolated();
      interpret(source, bounds, surface);
      interpret(backdrop, bounds, surface);
      surface.pop();
    }
    CompositeMode::SrcIn => interpret_masked(source, backdrop, bounds, surface),
    CompositeMode::DestIn => interpret_masked(backdrop, source, bounds, surface),
    _ => {
      // Approximate: `SrcOut`, `DestOut`, `SrcAtop`, `DestAtop`, `Xor`, `Plus`, and unknown modes
      // have no PDF blend mode, so they draw as `SrcOver`, where Skia composites them exactly.
      let Some(blend) = blend_mode(mode) else {
        surface.push_isolated();
        interpret(backdrop, bounds, surface);
        interpret(source, bounds, surface);
        surface.pop();
        return;
      };

      surface.push_isolated();
      interpret(backdrop, bounds, surface);
      surface.push_blend_mode(blend);
      surface.push_isolated();
      interpret(source, bounds, surface);
      surface.pop();
      surface.pop();
      surface.pop();
    }
  }
}

fn interpret_masked(
  visible: Vec<Instruction>,
  mask: Vec<Instruction>,
  bounds: &Path,
  surface: &mut Surface,
) {
  let mask_stream = {
    let mut builder = surface.stream_builder();
    let mut mask_surface = builder.surface();

    interpret(mask, bounds, &mut mask_surface);
    mask_surface.finish();
    builder.finish()
  };

  surface.push_mask(Mask::new(mask_stream, MaskType::Alpha));
  interpret(visible, bounds, surface);
  surface.pop();
}
