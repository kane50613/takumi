use skrifa::MetadataProvider;
use skrifa::color::{Brush, ColorPainter, ColorStop, CompositeMode};
use skrifa::outline::DrawSettings;
use skrifa::raw::TableProvider;
use skrifa::raw::types::BoundingBox;
use tiny_skia_path::{Path, PathBuilder};

use crate::krilla::geom::Transform;
use crate::krilla::graphics::blend::BlendMode;
use crate::krilla::graphics::color::rgb;
use crate::krilla::graphics::mask::{Mask, MaskType};
use crate::krilla::graphics::paint::{
  Fill, FillRule, LinearGradient, RadialGradient, SpreadMethod, Stop, SweepGradient,
};
use crate::krilla::num::NormalizedF32;
use crate::krilla::surface::Surface;
use crate::krilla::text::Font;
use crate::krilla::text::GlyphId;
use crate::krilla::text::outline::OutlineBuilder;

pub(crate) fn has_colr_data(font: &Font, glyph: GlyphId) -> bool {
  font
    .font_ref()
    .color_glyphs()
    .get(glyph.to_skrifa())
    .is_some()
}

/// Draw a COLR-based glyph on a surface.
pub(crate) fn draw_glyph(
  font: Font,
  context_color: rgb::Color,
  glyph: GlyphId,
  surface: &mut Surface,
) -> Option<()> {
  // Drawing COLR glyphs is a bit tricky, because it's possible that an error
  // occurs while we are drawing, in which case we cannot revert it anymore since
  // we already drew the instructions onto the surface. Because of this, we first
  // convert the glyph into a more accessible bytecode representation and only
  // if that succeeds do we iterate over the bytecode to draw onto the canvas.

  let colr_glyphs = font.font_ref().color_glyphs();
  let colr_glyph = colr_glyphs.get(glyph.to_skrifa())?;

  let mut colr_canvas = ColrBuilder::new(font.clone(), context_color);
  colr_glyph
    .paint(font.location_ref(), &mut colr_canvas)
    .ok()?;
  let instructions = colr_canvas.finish()?;

  surface.push_transform(&Transform::from_scale(1.0, -1.0));
  interpret(instructions, surface);
  surface.pop();

  Some(())
}

// Interpret the glyph bytecode
fn interpret(instructions: Vec<Instruction>, surface: &mut Surface) {
  for instruction in instructions {
    match instruction {
      Instruction::Layer(mode, instructions) => {
        let blend = mode.to_blend_mode().unwrap_or(BlendMode::Normal);
        surface.push_blend_mode(blend);
        surface.push_isolated();
        interpret(instructions, surface);
        surface.pop();
        surface.pop();
      }
      Instruction::Composite {
        source,
        mode,
        backdrop,
      } => interpret_composite(source, mode, backdrop, surface),
      Instruction::Filled(fill, mut clips) => {
        let filled = clips.split_off(clips.len() - 1);

        let num_clips = clips.len();
        for path in clips {
          surface.push_clip_path(&crate::krilla::geom::Path(path), &FillRule::NonZero);
        }

        let old_fill = surface.get_fill().cloned();
        let old_stroke = surface.get_stroke().cloned();

        surface.set_fill(Some(*fill));
        surface.set_stroke(None);

        surface.draw_path(&crate::krilla::geom::Path(filled[0].clone()));

        surface.set_fill(old_fill);
        surface.set_stroke(old_stroke);

        for _ in 0..num_clips {
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
  surface: &mut Surface,
) {
  match mode {
    CompositeMode::Clear => {}
    CompositeMode::Src => {
      surface.push_isolated();
      interpret(source, surface);
      surface.pop();
    }
    CompositeMode::Dest => {
      surface.push_isolated();
      interpret(backdrop, surface);
      surface.pop();
    }
    CompositeMode::SrcOver => {
      surface.push_isolated();
      interpret(backdrop, surface);
      interpret(source, surface);
      surface.pop();
    }
    CompositeMode::DestOver => {
      surface.push_isolated();
      interpret(source, surface);
      interpret(backdrop, surface);
      surface.pop();
    }
    CompositeMode::SrcIn => interpret_masked(source, backdrop, surface),
    CompositeMode::DestIn => interpret_masked(backdrop, source, surface),
    _ => {
      let Some(blend) = mode.to_blend_mode() else {
        surface.push_isolated();
        interpret(backdrop, surface);
        interpret(source, surface);
        surface.pop();
        return;
      };

      surface.push_isolated();
      interpret(backdrop, surface);
      surface.push_blend_mode(blend);
      surface.push_isolated();
      interpret(source, surface);
      surface.pop();
      surface.pop();
      surface.pop();
    }
  }
}

fn interpret_masked(visible: Vec<Instruction>, mask: Vec<Instruction>, surface: &mut Surface) {
  let mask_stream = {
    let mut builder = surface.stream_builder();
    let mut mask_surface = builder.surface();
    interpret(mask, &mut mask_surface);
    mask_surface.finish();
    builder.finish()
  };

  surface.push_mask(Mask::new(mask_stream, MaskType::Alpha));
  interpret(visible, surface);
  surface.pop();
}

/// The context necessary for creating the bytecode of a COLR-based glyph.
struct ColrBuilder {
  font: Font,
  context_color: rgb::Color,
  clips: Vec<Vec<Path>>,
  stack: Vec<Vec<Instruction>>,
  layers: Vec<CompositeMode>,
  transforms: Vec<Transform>,
  error: bool,
}

/// A bytecode instruction for drawing a COLR glyph.
enum Instruction {
  Layer(CompositeMode, Vec<Instruction>),
  Composite {
    source: Vec<Instruction>,
    mode: CompositeMode,
    backdrop: Vec<Instruction>,
  },
  Filled(Box<Fill>, Vec<Path>),
}

trait CompositeModeExt {
  fn to_blend_mode(self) -> Option<BlendMode>;
}

impl CompositeModeExt for CompositeMode {
  fn to_blend_mode(self) -> Option<BlendMode> {
    match self {
      CompositeMode::SrcOver => Some(BlendMode::Normal),
      CompositeMode::Screen => Some(BlendMode::Screen),
      CompositeMode::Overlay => Some(BlendMode::Overlay),
      CompositeMode::Darken => Some(BlendMode::Darken),
      CompositeMode::Lighten => Some(BlendMode::Lighten),
      CompositeMode::ColorDodge => Some(BlendMode::ColorDodge),
      CompositeMode::ColorBurn => Some(BlendMode::ColorBurn),
      CompositeMode::HardLight => Some(BlendMode::HardLight),
      CompositeMode::SoftLight => Some(BlendMode::SoftLight),
      CompositeMode::Difference => Some(BlendMode::Difference),
      CompositeMode::Exclusion => Some(BlendMode::Exclusion),
      CompositeMode::Multiply => Some(BlendMode::Multiply),
      CompositeMode::HslHue => Some(BlendMode::Hue),
      CompositeMode::HslColor => Some(BlendMode::Color),
      CompositeMode::HslLuminosity => Some(BlendMode::Luminosity),
      CompositeMode::HslSaturation => Some(BlendMode::Saturation),
      _ => None,
    }
  }
}

impl ColrBuilder {
  pub fn new(font: Font, context_color: rgb::Color) -> Self {
    Self {
      font,
      context_color,
      stack: vec![vec![]],
      transforms: vec![Transform::identity()],
      clips: vec![vec![]],
      layers: vec![],
      error: false,
    }
  }

  pub fn finish(mut self) -> Option<Vec<Instruction>> {
    if self.error {
      return None;
    } else if let Some(instructions) = self.stack.pop() {
      return Some(instructions);
    }

    None
  }
}

impl ColrBuilder {
  fn palette_index_to_color(
    &self,
    palette_index: u16,
    alpha: f32,
  ) -> Option<(rgb::Color, NormalizedF32)> {
    if palette_index != u16::MAX {
      let color = self
        .font
        .font_ref()
        .cpal()
        .ok()?
        .color_records_array()?
        .ok()?[palette_index as usize];

      Some((
        rgb::Color::new(color.red, color.green, color.blue),
        NormalizedF32::new(alpha * color.alpha as f32 / 255.0)?,
      ))
    } else {
      Some((self.context_color, NormalizedF32::new(alpha)?))
    }
  }

  fn stops(&self, stops: &[ColorStop]) -> Option<Vec<Stop>> {
    let mut converted_stops = vec![];

    for stop in stops {
      let (color, alpha) = self.palette_index_to_color(stop.palette_index, stop.alpha)?;

      converted_stops.push(Stop {
        offset: NormalizedF32::new(stop.offset)?,
        color: color.into(),
        opacity: alpha,
      })
    }

    Some(converted_stops)
  }
}

trait ExtendExt {
  fn to_spread_method(&self) -> SpreadMethod;
}

impl ExtendExt for skrifa::color::Extend {
  fn to_spread_method(&self) -> SpreadMethod {
    match self {
      skrifa::color::Extend::Pad => SpreadMethod::Pad,
      skrifa::color::Extend::Repeat => SpreadMethod::Repeat,
      skrifa::color::Extend::Reflect => SpreadMethod::Reflect,
      skrifa::color::Extend::Unknown => SpreadMethod::Pad,
    }
  }
}

impl ColorPainter for ColrBuilder {
  fn push_transform(&mut self, transform: skrifa::color::Transform) {
    let Some(last_transform) = self.transforms.last() else {
      self.error = true;
      return;
    };

    let new_transform = last_transform.pre_concat(Transform::from_row(
      transform.xx,
      transform.yx,
      transform.xy,
      transform.yy,
      transform.dx,
      transform.dy,
    ));
    self.transforms.push(new_transform);
  }

  fn pop_transform(&mut self) {
    let Some(_) = self.transforms.pop() else {
      self.error = true;
      return;
    };
  }

  fn push_clip_glyph(&mut self, glyph_id: skrifa::GlyphId) {
    let Some(mut old) = self.clips.last().cloned() else {
      self.error = true;
      return;
    };

    let mut glyph_builder = OutlineBuilder::new();
    let outline_glyphs = self.font.outline_glyphs();
    let Some(outline_glyph) = outline_glyphs.get(glyph_id) else {
      self.error = true;
      return;
    };

    let Ok(_) = outline_glyph.draw(
      DrawSettings::unhinted(skrifa::instance::Size::unscaled(), self.font.location_ref()),
      &mut takumi_core::resources::glyph::ErasedPen(&mut glyph_builder),
    ) else {
      self.error = true;
      return;
    };

    let Some(path) = glyph_builder
      .finish()
      .and_then(|p| p.transform(self.transforms.last()?.to_tsp()))
    else {
      self.error = true;
      return;
    };

    old.push(path);

    self.clips.push(old);
  }

  fn push_clip_box(&mut self, clip_box: BoundingBox<f32>) {
    let Some(mut old) = self.clips.last().cloned() else {
      self.error = true;
      return;
    };

    let mut path_builder = PathBuilder::new();
    path_builder.move_to(clip_box.x_min, clip_box.y_min);
    path_builder.line_to(clip_box.x_min, clip_box.y_max);
    path_builder.line_to(clip_box.x_max, clip_box.y_max);
    path_builder.line_to(clip_box.x_max, clip_box.y_min);
    path_builder.close();

    let Some(path) = path_builder
      .finish()
      .and_then(|p| p.transform(self.transforms.last()?.to_tsp()))
    else {
      self.error = true;
      return;
    };
    old.push(path);

    self.clips.push(old);
  }

  fn pop_clip(&mut self) {
    let Some(_) = self.clips.pop() else {
      self.error = true;
      return;
    };
  }

  fn fill(&mut self, brush: Brush<'_>) {
    if let Some(fill) = match brush {
      Brush::Solid {
        palette_index,
        alpha,
      } => {
        let (color, alpha) = match self.palette_index_to_color(palette_index, alpha) {
          Some(c) => c,
          None => {
            self.error = true;
            return;
          }
        };
        Some(Fill {
          paint: color.into(),
          opacity: alpha,
          rule: Default::default(),
        })
      }
      Brush::LinearGradient {
        p0,
        p1,
        color_stops,
        extend,
      } => {
        let stops = match self.stops(color_stops) {
          Some(s) => s,
          None => {
            self.error = true;
            return;
          }
        };

        let Some(transform) = self.transforms.last().copied() else {
          self.error = true;
          return;
        };

        let linear = LinearGradient {
          x1: p0.x,
          y1: p0.y,
          x2: p1.x,
          y2: p1.y,
          stops,
          spread_method: extend.to_spread_method(),
          transform,
          anti_alias: false,
        };

        Some(Fill {
          paint: linear.into(),
          opacity: NormalizedF32::ONE,
          rule: Default::default(),
        })
      }
      Brush::RadialGradient {
        c0,
        r0,
        c1,
        r1,
        color_stops,
        extend,
      } => {
        let stops = match self.stops(color_stops) {
          Some(s) => s,
          None => {
            self.error = true;
            return;
          }
        };

        let Some(transform) = self.transforms.last().copied() else {
          self.error = true;
          return;
        };

        let radial = RadialGradient {
          fx: c0.x,
          fy: c0.y,
          fr: r0,
          cx: c1.x,
          cy: c1.y,
          cr: r1,
          stops,
          spread_method: extend.to_spread_method(),
          transform,
          anti_alias: false,
        };

        Some(Fill {
          paint: radial.into(),
          opacity: NormalizedF32::ONE,
          rule: Default::default(),
        })
      }
      Brush::SweepGradient {
        c0,
        start_angle,
        end_angle,
        color_stops,
        extend,
      } => {
        let stops = match self.stops(color_stops) {
          Some(s) => s,
          None => {
            self.error = true;
            return;
          }
        };

        let Some(mut transform) = self.transforms.last().copied() else {
          self.error = true;
          return;
        };

        // krilla sweep gradients go in a different direction than COLR, so we need
        // to invert y-axis.

        transform = transform.pre_concat(Transform::from_scale(1.0, -1.0));

        let sweep = SweepGradient {
          cx: c0.x,
          cy: -c0.y,
          start_angle,
          end_angle,
          stops,
          spread_method: extend.to_spread_method(),
          transform,
          anti_alias: false,
        };

        Some(Fill {
          paint: sweep.into(),
          opacity: NormalizedF32::ONE,
          rule: Default::default(),
        })
      }
    } {
      // The proper implementation would be to apply all clip paths and then draw the
      // whole "visible" area with the fill. However, this seems to produce artifacts in
      // Google Chrome when zooming. So instead, what we do is that we apply all clip paths except
      // for the last one, and the last one we use to actually perform the fill.
      let Some(clips) = self.clips.last().map(|paths| paths.to_vec()) else {
        self.error = true;
        return;
      };

      let Some(stack) = self.stack.last_mut() else {
        self.error = true;
        return;
      };

      stack.push(Instruction::Filled(Box::new(fill), clips));
    }
  }

  fn push_layer(&mut self, composite_mode: CompositeMode) {
    self.layers.push(composite_mode);
    self.stack.push(vec![]);
  }

  fn pop_layer(&mut self) {
    let (Some(composite), Some(mut instructions)) = (self.layers.pop(), self.stack.pop()) else {
      self.error = true;
      return;
    };

    let Some(stack) = self.stack.last_mut() else {
      self.error = true;
      return;
    };

    // See the skrifa code to see how it handles composite operations.
    // https://github.com/googlefonts/fontations/blob/fd0178ce4ea48301a7f06b35a6fce9879c102292/skrifa/src/color/traversal.rs#L566-L593
    // Everything is wrapped in a `SrcOver` base layer.
    if composite == CompositeMode::SrcOver {
      if let Some(Instruction::Layer(_, _)) = instructions.last() {
        let Some(Instruction::Layer(mode, source)) = instructions.pop() else {
          unreachable!();
        };
        stack.push(Instruction::Composite {
          source,
          mode,
          backdrop: instructions,
        });
        return;
      }
    }

    stack.push(Instruction::Layer(composite, instructions));
  }
}
