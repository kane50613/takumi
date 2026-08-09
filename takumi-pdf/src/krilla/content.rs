//! A low-level abstraction over a single content stream.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use float_cmp::approx_eq;
use pdf_writer::types::TextRenderingMode;
use pdf_writer::{Content, Finish, Name, Ref, Str, TextStr};
use tiny_skia_path::{Path, PathSegment, PathVerb};

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::color::rgb;
use crate::krilla::configure::ValidationError;
#[cfg(feature = "raster-images")]
use crate::krilla::geom::Size;
use crate::krilla::geom::{Point, Rect, Transform};
use crate::krilla::graphics::color::{Color, ColorSpace};
use crate::krilla::graphics::graphics_state::{ExtGState, GraphicsStates};
#[cfg(feature = "raster-images")]
use crate::krilla::graphics::image::Image;
use crate::krilla::graphics::mask::Mask;
use crate::krilla::graphics::paint::{
  Fill, FillRule, InnerPaint, LineCap, LineJoin, Paint, Stroke,
};
use crate::krilla::graphics::shading_function::{
  GradientProperties, GradientPropertiesExt, ShadingFunction,
};
use crate::krilla::graphics::shading_pattern::ShadingPattern;
use crate::krilla::graphics::tiling_pattern::TilingPattern;
use crate::krilla::graphics::xobject::XObject;
use crate::krilla::interchange::tagging::ContentTag;
use crate::krilla::num::NormalizedF32;
use crate::krilla::resource;
use crate::krilla::resource::{Resource, ResourceDictionaryBuilder};
use crate::krilla::serialize::{MaybeDeviceColorSpace, SerializeContext};
use crate::krilla::stream::Stream;
use crate::krilla::text::group::{
  GlyphGroup, GlyphGrouper, GlyphSpan, GlyphSpanner, use_text_spanner,
};
use crate::krilla::text::type3::ColoredGlyph;
use crate::krilla::text::{Font, FontContainer, FontIdentifier, PDF_UNITS_PER_EM, PdfFont};
use crate::krilla::text::{Glyph, GlyphId};
use crate::krilla::util::{NameExt, calculate_stroke_bbox};

pub(crate) struct ContentBuilder {
  rd_builder: ResourceDictionaryBuilder,
  content: Content,
  validation_errors: HashSet<ValidationError>,
  root_transform: Transform,
  graphics_states: GraphicsStates,
  bbox: Option<Rect>,
  uses_mask: bool,
  // Calculating the bbox of text is expensive, so we should avoid doing it if not needed.
  // The only time we really need it is if we are currently inside of an XObject, where we
  // need to provide a bbox of all its contents. If we are on the main page stream, we only
  // need it if automatic size detection is enabled.
  bbox_important: bool,
  /// A temporary buffer that's reused across the builder.
  scratch: Vec<u8>,
  pub(crate) active_marked_content: bool,
}

/// Stores either a device-specific color space,
/// or the name of a different colorspace (e.g. ICCBased) stored in
/// the current resource dictionary
enum ContentColorSpace {
  Device,
  Named(String),
}

impl ContentBuilder {
  pub(crate) fn new(
    root_transform: Transform,
    bbox_important: bool,
    sc: &SerializeContext,
  ) -> Self {
    Self {
      rd_builder: ResourceDictionaryBuilder::new(),
      validation_errors: HashSet::new(),
      content: sc.new_content(),
      root_transform,
      uses_mask: false,
      bbox_important,
      graphics_states: GraphicsStates::new(),
      bbox: None,
      scratch: Vec::new(),
      active_marked_content: false,
    }
  }

  pub(crate) fn content_save_state(&mut self) {
    self.content.save_state();

    if self.content.state_nesting_depth() > 28 {
      self
        .validation_errors
        .insert(ValidationError::TooHighQNestingLevel);
    }
  }

  pub(crate) fn finish(self, sc: &mut SerializeContext) -> Stream {
    let buf = self.content.finish();
    sc.register_limits(buf.limits());

    Stream::new(
      buf.to_vec(),
      self
        .bbox
        .unwrap_or(Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap()),
      self.validation_errors.into_iter().collect(),
      self.rd_builder.finish(),
      self.uses_mask,
    )
  }

  #[track_caller]
  fn start_marked_content_prelude(&mut self) {
    if self.active_marked_content {
      panic!("can't start marked content twice");
    }

    self.active_marked_content = true;
  }

  #[track_caller]
  pub(crate) fn start_marked_content(&mut self, name: Name) {
    self.start_marked_content_prelude();
    self.content.begin_marked_content(name);
  }

  #[track_caller]
  pub(crate) fn start_marked_content_with_properties(
    &mut self,
    sc: &mut SerializeContext,
    mcid: Option<i32>,
    tag: ContentTag,
  ) {
    self.start_marked_content_prelude();

    let mut mc = self
      .content
      .begin_marked_content_with_properties(tag.name());
    let mut properties = mc.properties();

    if let Some(mcid) = mcid {
      properties.pairs([(Name(b"MCID"), mcid)]);
    }

    // Page height extracted from transform and passed to function to allow
    // its dependants to flip the y-axis, mirroring Krilla conventions.
    let page_height = self.root_transform.ty();
    tag.write_properties(sc, properties, page_height);
  }

  #[track_caller]
  pub(crate) fn end_marked_content(&mut self) {
    if !self.active_marked_content {
      panic!("can't end marked content when none has been started");
    }

    self.content.end_marked_content();
    self.active_marked_content = false;
  }

  pub(crate) fn concat_transform(&mut self, transform: &Transform) {
    self.graphics_states.transform(*transform);
  }

  fn cur_transform_with_root_transform(&self) -> Transform {
    self.root_transform.pre_concat(self.cur_transform())
  }

  pub(crate) fn cur_transform(&self) -> Transform {
    self.graphics_states.cur().transform()
  }

  pub(crate) fn save_graphics_state(&mut self) {
    self.graphics_states.save_state();
  }

  pub(crate) fn restore_graphics_state(&mut self) {
    self.graphics_states.restore_state();
  }

  pub(crate) fn set_blend_mode(&mut self, blend_mode: pdf_writer::types::BlendMode) {
    if blend_mode != pdf_writer::types::BlendMode::Normal {
      let state = ExtGState::new().blend_mode(blend_mode);
      self.graphics_states.combine(&state);
    }
  }

  pub(crate) fn expand_bbox(&mut self, new_bbox: Rect) {
    let new_bbox = self.graphics_states.transform_bbox(new_bbox);
    if let Some(bbox) = &mut self.bbox {
      bbox.expand(&new_bbox);
    } else {
      self.bbox = Some(new_bbox);
    }
  }

  pub(crate) fn draw_path(
    &mut self,
    path: &Path,
    mut fill: Option<&Fill>,
    stroke: Option<&Stroke>,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
  ) {
    if fill.is_none() && stroke.is_none() {
      return;
    }

    // Zero-size geometry, don't draw.
    if path.bounds().width() == 0.0 && path.bounds().height() == 0.0 {
      return;
    }

    // See issue 199.
    let is_line = path.verbs().len() == 2
      && path.verbs()[0] == PathVerb::Move
      && path.verbs()[1] == PathVerb::Line;
    let dont_fill = path.bounds().width() == 0.0 || path.bounds().height() == 0.0 || is_line;

    if dont_fill {
      match stroke.is_some() {
        // Some PDF viewers have bugs where they slightly fill zero-sized lines, so
        // don't draw them in the first place.
        false => return,
        // Don't fill, but we still want to draw the stroke, so don't return.
        true => {
          fill = None;
        }
      }
    }

    let bbox_important = self.bbox_important;
    let calculate_bbox = |is_solid: bool| bbox_important || !is_solid;

    let stroke_bbox = |stroke: &Stroke| {
      if calculate_bbox(matches!(&stroke.paint.0, InnerPaint::Color(_))) {
        calculate_stroke_bbox(stroke, path).unwrap_or(Rect::from_tsp(path.bounds()))
      } else {
        Rect::from_tsp(path.bounds())
      }
    };

    let fill_prep = |sb: &mut ContentBuilder, fill: &Fill| {
      let has_pattern = matches!(fill.paint.0, InnerPaint::Pattern(_));
      let fill_opacity = fill.opacity;
      sb.expand_bbox(Rect::from_tsp(path.bounds()));

      // PDF viewers don't show patterns with fill/stroke opacities consistently.
      // Because of this, the opacity is accounted for in the pattern itself.
      if !has_pattern {
        sb.set_fill_opacity(fill_opacity);
      }
    };

    let stroke_prep = |sb: &mut ContentBuilder, stroke: &Stroke, stroke_bbox: Rect| {
      let is_pattern = matches!(stroke.paint.0, InnerPaint::Pattern(_));
      let stroke_opacity = stroke.opacity;
      sb.expand_bbox(stroke_bbox);

      // See comment in `set_fill_properties`
      if !is_pattern {
        sb.set_stroke_opacity(stroke_opacity);
      }
    };

    let fill_op = |sb: &mut ContentBuilder,
                   sc: &mut SerializeContext,
                   chunk_container: &mut ChunkContainer,
                   fill: &Fill| {
      let fill_rule = fill.rule;
      sb.content_set_fill_properties(Rect::from_tsp(path.bounds()), fill, sc, chunk_container);
      sb.content_draw_path(path.segments());

      match fill_rule {
        FillRule::NonZero => sb.content.fill_nonzero(),
        FillRule::EvenOdd => sb.content.fill_even_odd(),
      };
    };

    let stroke_op = |sb: &mut ContentBuilder,
                     sc: &mut SerializeContext,
                     chunk_container: &mut ChunkContainer,
                     stroke: &Stroke,
                     stroke_bbox: Rect| {
      sb.content_set_stroke_properties(stroke_bbox, stroke, sc, chunk_container);
      sb.content_draw_path(path.segments());
      sb.content.stroke();
    };

    let fill_stroke_op = |sb: &mut ContentBuilder,
                          sc: &mut SerializeContext,
                          chunk_container: &mut ChunkContainer,
                          fill: &Fill,
                          stroke: &Stroke,
                          stroke_bbox: Rect| {
      let fill_rule = fill.rule;
      sb.content_set_fill_properties(Rect::from_tsp(path.bounds()), fill, sc, chunk_container);
      sb.content_set_stroke_properties(stroke_bbox, stroke, sc, chunk_container);
      sb.content_draw_path(path.segments());

      match fill_rule {
        FillRule::NonZero => sb.content.fill_nonzero_and_stroke(),
        FillRule::EvenOdd => sb.content.fill_even_odd_and_stroke(),
      };
    };

    match (fill, stroke) {
      (Some(fill), None) => {
        self.apply_isolated_op(
          |sb, _, _| {
            fill_prep(sb, fill);
          },
          |sb, sc, chunk_container| {
            fill_op(sb, sc, chunk_container, fill);
          },
          sc,
          chunk_container,
        );
      }
      (None, Some(stroke)) => {
        let stroke_bbox = stroke_bbox(stroke);

        self.apply_isolated_op(
          |sb, _, _| {
            stroke_prep(sb, stroke, stroke_bbox);
          },
          |sb, sc, chunk_container| {
            stroke_op(sb, sc, chunk_container, stroke, stroke_bbox);
          },
          sc,
          chunk_container,
        );
      }
      (Some(fill), Some(stroke)) => {
        let stroke_bbox = stroke_bbox(stroke);

        self.apply_isolated_op(
          |sb, _, _| {
            fill_prep(sb, fill);
            stroke_prep(sb, stroke, stroke_bbox);
          },
          |sb, sc, chunk_container| {
            fill_stroke_op(sb, sc, chunk_container, fill, stroke, stroke_bbox);
          },
          sc,
          chunk_container,
        );
      }
      (None, None) => unreachable!(),
    }
  }

  pub(crate) fn push_clip_path(&mut self, path: &Path, clip_rule: &FillRule) {
    self.content_save_state();
    self.content_draw_path(
      path
        .clone()
        .transform(self.cur_transform_with_root_transform().to_tsp())
        .unwrap()
        .segments(),
    );

    match clip_rule {
      FillRule::NonZero => self.content.clip_nonzero(),
      FillRule::EvenOdd => self.content.clip_even_odd(),
    };

    self.content.end_path();
  }

  pub(crate) fn pop_clip_path(&mut self) {
    self.content.restore_state();
  }

  #[allow(clippy::too_many_arguments)]
  pub(crate) fn draw_glyphs(
    &mut self,
    start: Point,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    fill: Option<&Fill>,
    stroke: Option<&Stroke>,
    context_color: rgb::Color,
    glyphs: &[impl Glyph],
    font: Font,
    text: &str,
    font_size: f32,
  ) {
    if fill.is_none() && stroke.is_none() {
      return;
    }

    let (x, y) = (start.x, start.y);
    self.graphics_states.save_state();

    // Calculating the glyphs bbox is very expensive but not always necessary, so omit
    // if not needed.
    let bbox_important = self.bbox_important;
    let calculate_bbox = |is_solid: bool| bbox_important || !is_solid;

    let fill_action = |sb: &mut ContentBuilder,
                       sc: &mut SerializeContext,
                       chunk_container: &mut ChunkContainer,
                       fill: &Fill| {
      let bbox = if calculate_bbox(matches!(&fill.paint.0, InnerPaint::Color(_))) {
        let bbox = get_glyphs_bbox(glyphs, x, y, font_size, font.clone());
        sb.expand_bbox(bbox);
        bbox
      } else {
        Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap()
      };

      sb.content_set_fill_properties(bbox, fill, sc, chunk_container)
    };

    let stroke_action = |sb: &mut ContentBuilder,
                         sc: &mut SerializeContext,
                         chunk_container: &mut ChunkContainer,
                         stroke: &Stroke| {
      let bbox = if calculate_bbox(matches!(&stroke.paint.0, InnerPaint::Color(_))) {
        // TODO: Bbox should also account for stroke.
        let bbox = get_glyphs_bbox(glyphs, x, y, font_size, font.clone());
        sb.expand_bbox(bbox);
        bbox
      } else {
        Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap()
      };

      sb.content_set_stroke_properties(bbox, stroke, sc, chunk_container);
    };

    let set_fill_opacity = |sb: &mut ContentBuilder, fill: &Fill| {
      // PDF viewers don't show patterns with fill/stroke opacities consistently.
      // Because of this, the opacity is accounted for in the pattern itself.
      if !matches!(&fill.paint.0, &InnerPaint::Pattern(_)) {
        sb.set_fill_opacity(fill.opacity);
      }
    };

    let set_stroke_opacity = |sb: &mut ContentBuilder, stroke: &Stroke| {
      // PDF viewers don't show patterns with fill/stroke opacities consistently.
      // Because of this, the opacity is accounted for in the pattern itself.
      if !matches!(&stroke.paint.0, &InnerPaint::Pattern(_)) {
        // PDF viewers don't show patterns with fill/stroke opacities consistently.
        // Because of this, the opacity is accounted for in the pattern itself.
        sb.set_stroke_opacity(stroke.opacity);

        // See the comment in `stroke-action` for why we also set the fill opacity.
        sb.set_fill_opacity(stroke.opacity);
      }
    };

    match (fill, stroke) {
      (Some(f), Some(s)) => {
        set_fill_opacity(self, f);
        set_stroke_opacity(self, s);

        self.fill_stroke_glyph_run(
          x,
          y,
          sc,
          chunk_container,
          TextRenderingMode::FillStroke,
          |sb, sc, chunk_container| {
            fill_action(sb, sc, chunk_container, f);
            stroke_action(sb, sc, chunk_container, s);
          },
          glyphs,
          font.clone(),
          context_color,
          text,
          font_size,
        );
      }
      (Some(f), None) => {
        set_fill_opacity(self, f);

        self.fill_stroke_glyph_run(
          x,
          y,
          sc,
          chunk_container,
          TextRenderingMode::Fill,
          |sb, sc, chunk_container| {
            fill_action(sb, sc, chunk_container, f);
          },
          glyphs,
          font.clone(),
          context_color,
          text,
          font_size,
        );
      }
      (None, Some(s)) => {
        set_stroke_opacity(self, s);

        self.fill_stroke_glyph_run(
          x,
          y,
          sc,
          chunk_container,
          TextRenderingMode::Stroke,
          |sb, sc, chunk_container| {
            stroke_action(sb, sc, chunk_container, s);
          },
          glyphs,
          font.clone(),
          context_color,
          text,
          font_size,
        );
      }
      (None, None) => unreachable!(),
    }

    self.graphics_states.restore_state();
  }

  /// Encode a successive sequence of glyphs that share the same properties and
  /// can be encoded with one text showing operator.
  #[allow(clippy::too_many_arguments)]
  #[inline(always)]
  fn encode_consecutive_glyph_run(
    &mut self,
    sc: &mut SerializeContext,
    cur_x: &mut f32,
    cur_y: f32,
    font_identifier: FontIdentifier,
    pdf_font: &dyn PdfFont,
    size: f32,
    context_color: rgb::Color,
    glyphs: &[impl Glyph],
    text: &str,
  ) {
    let font_name = self
      .rd_builder
      .register_resource(sc.register_font_identifier(font_identifier));
    self.content.set_font(font_name.to_pdf_name(), size);
    self
      .content
      .set_text_matrix(Transform::from_row(1.0, 0.0, 0.0, -1.0, *cur_x, cur_y).to_pdf_transform());

    for glyph in glyphs {
      if glyph.glyph_id() == GlyphId::new(0)
        || pdf_font.font().postscript_name() == Some("LastResort")
      {
        sc.register_validation_error(ValidationError::ContainsNotDefGlyph(
          pdf_font.font(),
          glyph.location(),
          text[glyph.text_range()].to_string(),
        ));
      }
    }

    // If it's just a single glyph and without an x_offset, there is no need for individual
    // positioning, so we can use `Tj` instead of `TJ`. The reason we're doing this is because
    // Acrobat's copy-paste is a bit buggy if `/ActualText` is used at the end of a line, but
    // only if `TJ` is used. With `Tj`, it's fine.
    if let [glyph] = glyphs {
      if approx_eq!(f32, glyph.x_offset(1.0), 0.0, epsilon = 0.001) {
        self.encode_single_glyph(cur_x, pdf_font, size, context_color, glyph);
        return;
      }
    }

    self.encode_glyphs_with_individual_positioning(cur_x, pdf_font, size, context_color, glyphs);
  }

  fn encode_single_glyph(
    &mut self,
    cur_x: &mut f32,
    pdf_font: &dyn PdfFont,
    size: f32,
    context_color: rgb::Color,
    glyph: &impl Glyph,
  ) {
    let pdf_glyph = pdf_font
      .get_gid(ColoredGlyph::new(glyph.glyph_id(), context_color))
      .unwrap();
    self.scratch.clear();
    let encoded = &mut self.scratch;
    pdf_glyph.encode_into(encoded);
    self.content.show(Str(encoded));
    *cur_x += glyph.x_advance(size);
  }

  fn encode_glyphs_with_individual_positioning(
    &mut self,
    cur_x: &mut f32,
    pdf_font: &dyn PdfFont,
    size: f32,
    context_color: rgb::Color,
    glyphs: &[impl Glyph],
  ) {
    self.scratch.clear();
    let encoded = &mut self.scratch;

    let mut adjustment = 0.0;
    let mut positioned = self.content.show_positioned();
    let mut items = positioned.items();

    for glyph in glyphs {
      let pdf_glyph = pdf_font
        .get_gid(ColoredGlyph::new(glyph.glyph_id(), context_color))
        .unwrap();

      let scale = |val| val * pdf_font.units_per_em();

      let x_advance = scale(glyph.x_advance(1.0));
      let font_advance = pdf_font
        .font()
        .advance_width(glyph.glyph_id())
        .map(|n| scale(n / pdf_font.font().units_per_em()));
      let x_offset = scale(glyph.x_offset(1.0));

      adjustment += x_offset;

      // Make sure we don't write miniscule adjustments
      if !approx_eq!(f32, adjustment, 0.0, epsilon = 0.001) {
        if !encoded.is_empty() {
          items.show(Str(encoded));
          encoded.clear();
        }

        // Adjustment is always in 1000 units, even for Type3 fonts.
        items.adjust(-(adjustment / pdf_font.units_per_em() * PDF_UNITS_PER_EM));
        adjustment = 0.0;
      }

      pdf_glyph.encode_into(encoded);

      if let Some(font_advance) = font_advance {
        adjustment += x_advance - font_advance;
      }

      adjustment -= x_offset;
      // cur_x/cur_y and glyph metrics are in user space units.
      *cur_x += glyph.x_advance(size);
    }

    if !encoded.is_empty() {
      items.show(Str(encoded));
    }

    items.finish();
    positioned.finish();
  }

  #[allow(clippy::too_many_arguments)]
  fn fill_stroke_glyph_run(
    &mut self,
    x: f32,
    ys: f32,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    fill_render_mode: TextRenderingMode,
    action: impl FnOnce(&mut ContentBuilder, &mut SerializeContext, &mut ChunkContainer),
    glyphs: &[impl Glyph],
    font: Font,
    context_color: rgb::Color,
    text: &str,
    font_size: f32,
  ) {
    if glyphs.is_empty() {
      return;
    }

    self.apply_isolated_op(
      |_, _, _| {},
      |sb, sc, chunk_container| {
        let mut cur_x = x;
        let mut cur_y = ys;

        // When the glyph ranges are monotonically decreasing, the glyphs are in reverse
        // order and we should write `/ReversedChars` marked content.
        let reversed = glyphs.len() > 1
          && glyphs
            .windows(2)
            .all(|w| w[0].text_range().start >= w[1].text_range().end);

        action(sb, sc, chunk_container);
        sb.content.begin_text();

        let font_container = sc.register_font_container(font.clone());
        let do_text_span = reversed
          || use_text_spanner(
            glyphs,
            text,
            context_color,
            &mut font_container.borrow_mut(),
          );

        if do_text_span {
          // Separate into distinct glyph runs that either are encoded using actual text, or are
          // not.
          // Every glyph gets a codepoint, not just the first of a cluster. A
          // viewer without `/ActualText` support otherwise reads the subset
          // glyph index for the rest of the cluster.
          let spanned = GlyphSpanner::new(
            glyphs,
            text,
            reversed,
            true,
            context_color,
            font_container.clone(),
          );

          for fragment in spanned {
            sb.fill_stroke_glyph_span(
              &mut cur_x,
              &mut cur_y,
              fragment,
              sc,
              fill_render_mode,
              font_container.clone(),
              context_color,
              text,
              font_size,
              reversed,
            )
          }
        } else {
          let glyph_span = GlyphSpan::Unspanned(glyphs);

          sb.fill_stroke_glyph_span(
            &mut cur_x,
            &mut cur_y,
            glyph_span,
            sc,
            fill_render_mode,
            font_container.clone(),
            context_color,
            text,
            font_size,
            reversed,
          )
        }

        sb.content.end_text();
      },
      sc,
      chunk_container,
    )
  }

  #[allow(clippy::too_many_arguments)]
  fn fill_stroke_glyph_span(
    &mut self,
    cur_x: &mut f32,
    cur_y: &mut f32,
    fragment: GlyphSpan<'_, impl Glyph>,
    sc: &mut SerializeContext,
    fill_render_mode: TextRenderingMode,
    font_container: Rc<RefCell<FontContainer>>,
    context_color: rgb::Color,
    text: &str,
    font_size: f32,
    reversed: bool,
  ) {
    if let Some(text) = fragment.actual_text() {
      let mut actual_text = self
        .content
        .begin_marked_content_with_properties(Name(b"Span"));
      actual_text.properties().actual_text(TextStr(text));
    }

    if reversed {
      self.content.begin_marked_content(Name(b"ReversedChars"));
    }

    // Segment into glyph runs that can be encoded in one go using a PDF
    // text showing operator (i.e. no y shift, same Type3 font, etc.)
    let segmented = GlyphGrouper::new(font_container.clone(), context_color, fragment.glyphs());

    for glyph_group in segmented {
      self.fill_stroke_glyph_group(
        cur_x,
        cur_y,
        glyph_group,
        sc,
        fill_render_mode,
        font_container.clone(),
        context_color,
        text,
        font_size,
      )
    }

    if reversed {
      self.content.end_marked_content();
    }

    if fragment.actual_text().is_some() {
      self.content.end_marked_content();
    }
  }

  #[allow(clippy::too_many_arguments)]
  fn fill_stroke_glyph_group(
    &mut self,
    cur_x: &mut f32,
    cur_y: &mut f32,
    glyph_group: GlyphGroup<'_, impl Glyph>,
    sc: &mut SerializeContext,
    fill_render_mode: TextRenderingMode,
    font_container: Rc<RefCell<FontContainer>>,
    context_color: rgb::Color,
    text: &str,
    font_size: f32,
  ) {
    let borrowed = font_container.borrow();
    let pdf_font = borrowed
      .get_from_identifier(glyph_group.font_identifier.clone())
      .unwrap();

    if fill_render_mode == TextRenderingMode::Fill || pdf_font.force_fill() {
      self
        .content
        .set_text_rendering_mode(TextRenderingMode::Fill);
    } else if fill_render_mode == TextRenderingMode::FillStroke {
      self
        .content
        .set_text_rendering_mode(TextRenderingMode::FillStroke);
    } else {
      self
        .content
        .set_text_rendering_mode(TextRenderingMode::Stroke);
    }

    self.encode_consecutive_glyph_run(
      sc,
      cur_x,
      *cur_y - glyph_group.y_offset * font_size,
      glyph_group.font_identifier,
      pdf_font,
      font_size,
      context_color,
      glyph_group.glyphs,
      text,
    );

    *cur_y -= glyph_group.y_advance * font_size;
  }

  pub(crate) fn draw_xobject(
    &mut self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    x_object: XObject,
    state: &ExtGState,
  ) {
    let bbox = x_object.bbox();
    self.apply_isolated_op(
      |sb, _, _| {
        sb.graphics_states.combine(state);
        sb.expand_bbox(bbox);
      },
      move |sb, sc, chunk_container| {
        let x_object_name = sb
          .rd_builder
          .register_resource(sc.register_resourceable(chunk_container, x_object));
        sb.content.x_object(x_object_name.to_pdf_name());
      },
      sc,
      chunk_container,
    );
  }

  #[allow(dead_code)]
  pub(crate) fn draw_xobject_by_reference(
    &mut self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    bbox: Rect,
    x_object: Ref,
  ) {
    // TODO: Consider bbox of XObject somehow?
    self.apply_isolated_op(
      |sb, _, _| {
        sb.expand_bbox(bbox);
      },
      move |sb, _, _| {
        let x_object_name = sb
          .rd_builder
          .register_resource(resource::XObject::new(x_object));
        sb.content.x_object(x_object_name.to_pdf_name());
      },
      sc,
      chunk_container,
    );
  }

  pub(crate) fn draw_masked(
    &mut self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    mask: Mask,
    stream: Stream,
  ) {
    let state = ExtGState::new().mask(mask, sc, chunk_container);
    self.uses_mask = true;
    let x_object = XObject::new(stream, false, true, None);
    self.draw_xobject(sc, chunk_container, x_object, &state);
  }

  pub(crate) fn draw_opacified(
    &mut self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    opacity: NormalizedF32,
    stream: Stream,
  ) {
    let state = ExtGState::new()
      .stroking_alpha(opacity)
      .non_stroking_alpha(opacity);
    let x_object = XObject::new(stream, true, false, None);
    self.draw_xobject(sc, chunk_container, x_object, &state);
  }

  pub(crate) fn draw_isolated(
    &mut self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    stream: Stream,
  ) {
    let state = ExtGState::new();
    let x_object = XObject::new(stream, true, false, None);
    self.draw_xobject(sc, chunk_container, x_object, &state);
  }

  #[cfg(feature = "raster-images")]
  pub(crate) fn draw_image(
    &mut self,
    image: Image,
    size: Size,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
  ) {
    self.apply_isolated_op(
      |sb, _, _| {
        // Scale the image from 1x1 to the actual dimensions.
        let transform =
          Transform::from_row(size.width(), 0.0, 0.0, -size.height(), 0.0, size.height());
        sb.concat_transform(&transform);
        sb.expand_bbox(Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap());
      },
      move |sb, sc, chunk_container| {
        let image_name = sb.rd_builder.register_resource(resource::XObject::new(
          sc.register_image(chunk_container, image),
        ));

        sb.content.x_object(image_name.to_pdf_name());
      },
      sc,
      chunk_container,
    );
  }

  pub(crate) fn draw_shading(
    &mut self,
    shading: &ShadingFunction,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
  ) {
    self.apply_isolated_op(
      |_, _, _| {},
      move |sb, sc, chunk_container| {
        let sh = sb
          .rd_builder
          .register_resource(sc.register_resourceable(chunk_container, shading.clone()));
        sb.content.shading(sh.to_pdf_name());
      },
      sc,
      chunk_container,
    )
  }

  fn set_fill_opacity(&mut self, alpha: NormalizedF32) {
    if alpha.get() != 1.0 {
      let state = ExtGState::new().non_stroking_alpha(alpha);
      self.graphics_states.combine(&state);
    }
  }

  fn set_stroke_opacity(&mut self, alpha: NormalizedF32) {
    if alpha.get() != 1.0 {
      let state = ExtGState::new().stroking_alpha(alpha);
      self.graphics_states.combine(&state);
    }
  }

  fn apply_isolated_op(
    &mut self,
    prep: impl FnOnce(&mut Self, &mut SerializeContext, &mut ChunkContainer),
    op: impl FnOnce(&mut Self, &mut SerializeContext, &mut ChunkContainer),
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
  ) {
    self.save_graphics_state();
    self.content_save_state();

    prep(self, sc, chunk_container);

    let transform = self.cur_transform_with_root_transform();

    if transform != Transform::identity() {
      self.content.transform(transform.to_pdf_transform());
    }

    let state = self.graphics_states.cur().ext_g_state().clone();

    if !state.empty() {
      let ext = self
        .rd_builder
        .register_resource::<resource::ExtGState>(sc.register_resourceable(chunk_container, state));
      self.content.set_parameters(ext.to_pdf_name());
    }

    op(self, sc, chunk_container);

    self.content.restore_state();
    self.restore_graphics_state();
  }

  #[allow(clippy::too_many_arguments)]
  fn content_set_fill_stroke_properties(
    &mut self,
    bounds: Rect,
    paint: &Paint,
    opacity: NormalizedF32,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    mut set_pattern_fn: impl FnMut(&mut Content, String),
    mut set_solid_fn: impl FnMut(&mut Content, ContentColorSpace, &Color),
  ) {
    let pattern_transform = |transform: Transform| -> Transform {
      transform.post_concat(self.cur_transform_with_root_transform())
    };

    let mut write_gradient = |gradient_props: GradientProperties,
                              sc: &mut SerializeContext,
                              chunk_container: &mut ChunkContainer,
                              transform: Transform,
                              content_builder: &mut ContentBuilder| {
      if let Some((color, opacity)) = gradient_props.single_stop_color() {
        // Write gradients with one stop as a solid color fill.
        content_builder.set_fill_opacity(opacity);
        let cs = color.color_space(sc);
        let color_space_resource = Self::cs_to_content_cs(content_builder, sc, chunk_container, cs);
        set_solid_fn(&mut content_builder.content, color_space_resource, color);
      } else {
        let shading_mask = Mask::new_from_shading(
          gradient_props.clone(),
          transform,
          bounds,
          sc,
          chunk_container,
        );

        let shading_pattern = ShadingPattern::new(
          gradient_props,
          content_builder
            .cur_transform_with_root_transform()
            .pre_concat(transform),
        );
        let color_space = content_builder
          .rd_builder
          .register_resource::<resource::Pattern>(
            sc.register_resourceable(chunk_container, shading_pattern),
          );

        if let Some(shading_mask) = shading_mask {
          let state = ExtGState::new().mask(shading_mask, sc, chunk_container);
          content_builder.uses_mask = true;

          let ext = content_builder
            .rd_builder
            .register_resource::<resource::ExtGState>(
              sc.register_resourceable(chunk_container, state),
            );
          content_builder.content.set_parameters(ext.to_pdf_name());
        }

        set_pattern_fn(&mut content_builder.content, color_space);
      }
    };

    match &paint.0 {
      InnerPaint::Color(c) => {
        let cs = c.color_space(sc);
        let color_space_resource = Self::cs_to_content_cs(self, sc, chunk_container, cs);
        set_solid_fn(&mut self.content, color_space_resource, c);
      }
      InnerPaint::LinearGradient(lg) => {
        let (gradient_props, transform) = lg.clone().gradient_properties(bounds);
        write_gradient(gradient_props, sc, chunk_container, transform, self);
      }
      InnerPaint::RadialGradient(rg) => {
        let (gradient_props, transform) = rg.clone().gradient_properties(bounds);
        write_gradient(gradient_props, sc, chunk_container, transform, self);
      }
      InnerPaint::SweepGradient(sg) => {
        let (gradient_props, transform) = sg.clone().gradient_properties(bounds);
        write_gradient(gradient_props, sc, chunk_container, transform, self);
      }
      InnerPaint::Pattern(pat) => {
        let mut pat = Arc::unwrap_or_clone(pat.clone());
        pat.transform = pattern_transform(pat.transform);

        let tiling_pattern = TilingPattern::new(
          pat.stream,
          pat.transform,
          opacity,
          pat.width,
          pat.height,
          sc,
          chunk_container,
        );

        let color_space = self.rd_builder.register_resource::<resource::Pattern>(
          sc.register_resourceable(chunk_container, tiling_pattern),
        );
        set_pattern_fn(&mut self.content, color_space);
      }
    }
  }

  fn cs_to_content_cs(
    content_builder: &mut ContentBuilder,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    cs: ColorSpace,
  ) -> ContentColorSpace {
    match sc.register_colorspace(chunk_container, cs) {
      MaybeDeviceColorSpace::ColorSpace(s) => {
        ContentColorSpace::Named(content_builder.rd_builder.register_resource(s))
      }
      MaybeDeviceColorSpace::DeviceGray => ContentColorSpace::Device,
      MaybeDeviceColorSpace::DeviceRgb => ContentColorSpace::Device,
      MaybeDeviceColorSpace::DeviceCMYK => ContentColorSpace::Device,
    }
  }

  fn content_set_fill_properties(
    &mut self,
    bounds: Rect,
    fill: &Fill,
    serializer_context: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
  ) {
    fn set_pattern_fn(content: &mut Content, color_space: String) {
      content.set_fill_color_space(pdf_writer::types::ColorSpaceOperand::Pattern);
      content.set_fill_pattern(None, color_space.to_pdf_name());
    }

    fn set_solid_fn(content: &mut Content, color_space: ContentColorSpace, color: &Color) {
      match color_space {
        ContentColorSpace::Device => match color {
          Color::Regular(crate::krilla::color::RegularColor::Rgb(r)) => {
            let comps = r.to_pdf_color();
            content.set_fill_rgb(comps[0], comps[1], comps[2]);
          }
          Color::Regular(crate::krilla::color::RegularColor::Luma(l)) => {
            content.set_fill_gray(l.to_pdf_color());
          }
          Color::Regular(crate::krilla::color::RegularColor::Cmyk(c)) => {
            let comps = c.to_pdf_color();
            content.set_fill_cmyk(comps[0], comps[1], comps[2], comps[3]);
          }
          Color::Special(_) => {
            panic!("Device color space cannot be used with special colors")
          }
        },
        ContentColorSpace::Named(n) => {
          content.set_fill_color_space(n.to_pdf_name());
          content.set_fill_color(color.to_pdf_color());
        }
      }
    }

    self.content_set_fill_stroke_properties(
      bounds,
      &fill.paint,
      fill.opacity,
      serializer_context,
      chunk_container,
      set_pattern_fn,
      set_solid_fn,
    );
  }

  fn content_set_stroke_properties(
    &mut self,
    bounds: Rect,
    stroke: &Stroke,
    serializer_context: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
  ) {
    fn set_pattern_fn(content: &mut Content, color_space: String) {
      content.set_stroke_color_space(pdf_writer::types::ColorSpaceOperand::Pattern);
      content.set_stroke_pattern(None, color_space.to_pdf_name());
    }

    fn set_solid_fn(content: &mut Content, color_space: ContentColorSpace, color: &Color) {
      match color_space {
        ContentColorSpace::Device => match color {
          Color::Regular(crate::krilla::color::RegularColor::Rgb(r)) => {
            let comps = r.to_pdf_color();
            content.set_stroke_rgb(comps[0], comps[1], comps[2]);
          }
          Color::Regular(crate::krilla::color::RegularColor::Luma(l)) => {
            content.set_stroke_gray(l.to_pdf_color());
          }
          Color::Regular(crate::krilla::color::RegularColor::Cmyk(c)) => {
            let comps = c.to_pdf_color();
            content.set_stroke_cmyk(comps[0], comps[1], comps[2], comps[3]);
          }
          Color::Special(_) => {
            panic!("Device color space cannot be used with special colors")
          }
        },
        ContentColorSpace::Named(n) => {
          content.set_stroke_color_space(n.to_pdf_name());
          content.set_stroke_color(color.to_pdf_color());
        }
      }
    }

    self.content_set_fill_stroke_properties(
      bounds,
      &stroke.paint,
      stroke.opacity,
      serializer_context,
      chunk_container,
      set_pattern_fn,
      set_solid_fn,
    );

    // Only write if they don't correspond to the default values as defined in the
    // PDF specification.
    if stroke.width != 1.0 {
      self.content.set_line_width(stroke.width);
    }

    if stroke.miter_limit != 10.0 {
      self.content.set_miter_limit(stroke.miter_limit);
    }

    if stroke.line_cap != LineCap::Butt {
      self.content.set_line_cap(stroke.line_cap.to_pdf_line_cap());
    }

    if stroke.line_join != LineJoin::Miter {
      self
        .content
        .set_line_join(stroke.line_join.to_pdf_line_join());
    }

    if let Some(stroke_dash) = &stroke.dash {
      self
        .content
        .set_dash_pattern(stroke_dash.array.iter().copied(), stroke_dash.offset);
    }
  }

  fn content_draw_path(&mut self, path_data: impl Iterator<Item = PathSegment>) {
    // Taken from resvg
    fn calc(n1: f32, n2: f32) -> f32 {
      (n1 + n2 * 2.0) / 3.0
    }

    let mut p_prev = None;

    for operation in simplify_path(path_data) {
      let operation = match operation {
        PathOp::Rect { x, y, w, h } => {
          self.content.rect(x, y, w, h);
          continue;
        }
        PathOp::Segment(segment) => segment,
      };

      match operation {
        PathSegment::MoveTo(p) => {
          self.content.move_to(p.x, p.y);
          p_prev = Some(p);
        }
        PathSegment::LineTo(p) => {
          self.content.line_to(p.x, p.y);
          p_prev = Some(p);
        }
        PathSegment::QuadTo(p1, p2) => {
          // Since PDF doesn't support quad curves, we need to convert them into
          // cubic.
          let prev = p_prev.unwrap();
          self.content.cubic_to(
            calc(prev.x, p1.x),
            calc(prev.y, p1.y),
            calc(p2.x, p1.x),
            calc(p2.y, p1.y),
            p2.x,
            p2.y,
          );
          p_prev = Some(p2);
        }
        PathSegment::CubicTo(p1, p2, p3) => {
          self.content.cubic_to(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y);
          p_prev = Some(p3);
        }
        PathSegment::Close => {
          self.content.close_path();
        }
      };
    }
  }
}

/// One drawing operation of a simplified path.
enum PathOp {
  Segment(PathSegment),
  /// An axis-aligned rectangle, which `re` writes in four numbers instead of
  /// four operators.
  Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
  },
}

/// Rewrites a path into the shortest operators that paint the same region.
///
/// Box decorations arrive from the shared geometry builder with every corner
/// point repeated, from the zero-radius arcs, and with the closing edge written
/// out before the close. Neither paints anything. What is left of a rectangle is
/// then four points, which `re` states in one operator.
fn simplify_path(path_data: impl Iterator<Item = PathSegment>) -> Vec<PathOp> {
  let mut out = Vec::new();
  let mut subpath: Vec<PathSegment> = Vec::new();

  let flush = |subpath: &mut Vec<PathSegment>, out: &mut Vec<PathOp>, closed: bool| {
    // `re` appends a closed subpath, so it stands in for the close as well.
    if closed && let Some(rect) = as_rect(subpath) {
      subpath.clear();
      out.push(rect);
      return;
    }

    out.extend(subpath.drain(..).map(PathOp::Segment));

    if closed {
      out.push(PathOp::Segment(PathSegment::Close));
    }
  };

  for segment in path_data {
    match segment {
      PathSegment::MoveTo(_) => {
        flush(&mut subpath, &mut out, false);
        subpath.push(segment);
      }
      // A segment that goes nowhere.
      PathSegment::LineTo(p) if subpath.last().and_then(end_point) == Some(p) => {}
      PathSegment::Close => {
        // `h` draws the closing edge itself.
        if let (Some(PathSegment::MoveTo(start)), Some(PathSegment::LineTo(last))) =
          (subpath.first().copied(), subpath.last().copied())
          && start == last
        {
          subpath.pop();
        }

        flush(&mut subpath, &mut out, true);
      }
      _ => subpath.push(segment),
    }
  }

  flush(&mut subpath, &mut out, false);
  out
}

fn end_point(segment: &PathSegment) -> Option<tiny_skia_path::Point> {
  match *segment {
    PathSegment::MoveTo(p) | PathSegment::LineTo(p) => Some(p),
    PathSegment::QuadTo(_, p) | PathSegment::CubicTo(_, _, p) => Some(p),
    PathSegment::Close => None,
  }
}

/// Matches a subpath that `re` traces point for point: a horizontal edge, then a
/// vertical one, then a horizontal one back. Any other winding or start point
/// stays as it is, so the fill rule and the dash phase cannot shift.
fn as_rect(subpath: &[PathSegment]) -> Option<PathOp> {
  let [
    PathSegment::MoveTo(p0),
    PathSegment::LineTo(p1),
    PathSegment::LineTo(p2),
    PathSegment::LineTo(p3),
  ] = subpath
  else {
    return None;
  };

  (p0.y == p1.y && p1.x == p2.x && p2.y == p3.y && p3.x == p0.x).then_some(PathOp::Rect {
    x: p0.x,
    y: p0.y,
    w: p1.x - p0.x,
    h: p2.y - p1.y,
  })
}

// Note that this isn't a 100% accurate calculation, it can overestimate (and in a few cases
// even underestimate), but it should be good enough for the majority of the cases.
// TODO: Improve this so that `zalgo_text` test case shows up fully in the reference image.
fn get_glyphs_bbox(glyphs: &[impl Glyph], x: f32, y: f32, size: f32, font: Font) -> Rect {
  let font_bbox = font.bbox();
  let (mut bl, mut bt, mut br, mut bb) = font_bbox
    .transform(Transform::from_scale(
      size / font.units_per_em(),
      -size / font.units_per_em(),
    ))
    .and_then(|b| b.transform(Transform::from_translate(x, y)))
    .map(|b| (b.left(), b.top(), b.right(), b.bottom()))
    .unwrap_or((x, y, x + 1.0, y + 1.0));

  let mut x = x;
  let mut y = y;

  for glyph in glyphs {
    let xo = glyph.x_offset(size);
    let xa = glyph.x_advance(size);
    let yo = glyph.y_offset(size);
    let ya = glyph.y_advance(size);

    x += xa;
    y -= ya;

    bl = bl.min(x + xo);
    br = br.max(x + xo);
    bt = bt.min(y - yo);
    bb = bb.max(y - yo);
  }

  Rect::from_ltrb(bl, bt, br, bb).unwrap()
}

#[cfg(test)]
mod tests {
  use super::{PathOp, simplify_path};

  use tiny_skia_path::{PathSegment, Point};

  fn line(x: f32, y: f32) -> PathSegment {
    PathSegment::LineTo(Point::from_xy(x, y))
  }

  fn start(x: f32, y: f32) -> PathSegment {
    PathSegment::MoveTo(Point::from_xy(x, y))
  }

  fn describe(segments: Vec<PathSegment>) -> Vec<String> {
    simplify_path(segments.into_iter())
      .into_iter()
      .map(|op| match op {
        PathOp::Rect { x, y, w, h } => format!("re {x} {y} {w} {h}"),
        PathOp::Segment(PathSegment::MoveTo(p)) => format!("m {} {}", p.x, p.y),
        PathOp::Segment(PathSegment::LineTo(p)) => format!("l {} {}", p.x, p.y),
        PathOp::Segment(PathSegment::Close) => "h".to_string(),
        PathOp::Segment(_) => "curve".to_string(),
      })
      .collect()
  }

  #[test]
  fn a_rectangle_with_repeated_corners_collapses_to_one_operator() {
    let path = vec![
      start(36.0, 55.0),
      line(359.0, 55.0),
      line(359.0, 55.0),
      line(359.0, 57.0),
      line(359.0, 57.0),
      line(36.0, 57.0),
      line(36.0, 57.0),
      line(36.0, 55.0),
      PathSegment::Close,
    ];

    assert_eq!(describe(path), ["re 36 55 323 2"]);
  }

  #[test]
  fn a_rectangle_traced_the_other_way_stays_a_path() {
    let path = vec![
      start(0.0, 0.0),
      line(0.0, 10.0),
      line(20.0, 10.0),
      line(20.0, 0.0),
      PathSegment::Close,
    ];

    assert_eq!(
      describe(path),
      ["m 0 0", "l 0 10", "l 20 10", "l 20 0", "h"]
    );
  }

  #[test]
  fn an_open_path_keeps_its_closing_edge() {
    let path = vec![start(0.0, 0.0), line(10.0, 0.0), line(0.0, 0.0)];

    assert_eq!(describe(path), ["m 0 0", "l 10 0", "l 0 0"]);
  }
}
