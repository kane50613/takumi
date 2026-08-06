//! Drawing onto a surface.
//!
//! This module contains most core part of krilla: The [`Surface`] struct. A surface
//! represents a drawing area on which you can define the contents of your page. This includes
//! operations such as applying linear transformations, showing text or images and drawing paths.

use std::num::NonZeroU64;

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::color::rgb;
use crate::krilla::configure::ValidationError;
use crate::krilla::configure::validate::VersionedFeature;
use crate::krilla::content::ContentBuilder;
use crate::krilla::geom::Path;
#[cfg(feature = "raster-images")]
use crate::krilla::geom::Size;
use crate::krilla::geom::{Point, Transform};
use crate::krilla::graphic::Graphic;
use crate::krilla::graphics::blend::BlendMode;
use crate::krilla::graphics::graphics_state::ExtGState;
#[cfg(feature = "raster-images")]
use crate::krilla::graphics::image::Image;
use crate::krilla::graphics::mask::Mask;
use crate::krilla::graphics::paint::{Fill, FillRule, Stroke};
use crate::krilla::graphics::shading_function::ShadingFunction;
use crate::krilla::interchange::tagging::{ContentTag, Identifier, PageTagIdentifier};
use crate::krilla::num::NormalizedF32;
use crate::krilla::paint::{InnerPaint, Paint};
use crate::krilla::serialize::SerializeContext;
use crate::krilla::stream::{Stream, StreamBuilder};
use crate::krilla::tagging::ArtifactType;
use crate::krilla::text::Font;
use crate::krilla::text::{Glyph, draw_glyph};

/// Can be used to associate render operations with a unique identifier.
/// This is useful if you want to backtrack a validation error to a specific
/// identifier chosen by you.
pub type Location = NonZeroU64;

/// A surface.
///
/// Represents a drawing area for defining graphical content. The origin of the
/// coordinate axis is in the top-left corner.
///
/// You cannot directly create an instance of a [`Surface`] yourself.
/// Instead, there are two ways of getting access to a surface, which you can then use to draw on:
///
/// - The first way, and also the most common one you will use, is by creating a new document,
///   adding a page to it and then invoking the [`Page::surface`] method. The surface returned as part of
///   that represents the drawing area of the page.
/// - The second way is by calling the [`Surface::stream_builder`] method on the surface, to create a sub-drawing
///   context. See the documentation of the [`stream`] module for more information.
///
/// The surface uses a `push` and `pop` based mechanism for applying certain actions. For example,
/// there is a [`Surface::push_transform`] method which allows you to concatenate a new transform to the
/// current transform matrix. There is also a [`Surface::push_clip_path`] method, which allows you to
/// intersect the current drawing area with a clip path. Once you call such a `push` method,
/// the action that it invokes will be in place until you call the `pop` method, which will then
/// revert the last `push` operation. This allows you to, for example, define a clipping path area
/// or a mask to use only for certain objects.
///
/// It is important that, for each `push` method you invoke, there must be a corresponding `pop`
/// invocation that reverts it. If, at the end of the using the surface, the number of pushes/pops is
/// uneven, the program will panic.
///
/// [`stream`]: crate::krilla::stream
/// [`Page::surface`]: crate::krilla::page::Page::surface
pub struct Surface<'a> {
  pub(crate) sc: &'a mut SerializeContext,
  pub(crate) chunk_container: &'a mut ChunkContainer,
  fill: Option<Fill>,
  stroke: Option<Stroke>,
  bd: Builders,
  push_instructions: Vec<PushInstruction>,
  page_identifier: Option<PageTagIdentifier>,
  finish_fn: Box<dyn FnMut(Stream, i32) + 'a>,
}

impl<'a> Surface<'a> {
  pub(crate) fn new(
    sc: &'a mut SerializeContext,
    chunk_container: &'a mut ChunkContainer,
    root_builder: ContentBuilder,
    page_identifier: Option<PageTagIdentifier>,
    finish_fn: Box<dyn FnMut(Stream, i32) + 'a>,
  ) -> Surface<'a> {
    Self {
      sc,
      chunk_container,
      bd: Builders::new(root_builder),
      page_identifier,
      fill: None,
      stroke: None,
      push_instructions: vec![],
      finish_fn,
    }
  }

  /// Return a `StreamBuilder` to allow drawing on a sub-context.
  pub fn stream_builder(&mut self) -> StreamBuilder<'_> {
    StreamBuilder::new(self.sc, self.chunk_container)
  }

  /// Set the fill that should be used for the next drawing operation.
  ///
  /// You can set it to `None` if you want to disable filling (though
  /// if there is no active stroke, than krilla will choose to fill
  /// with black by default).
  pub fn set_fill(&mut self, fill: Option<Fill>) {
    self.fill = fill;
  }

  /// Get the currently active fill.
  pub fn get_fill(&mut self) -> Option<&Fill> {
    self.fill.as_ref()
  }

  /// Set the stroke that should be used for drawing operations.
  ///
  /// You can set it to `None` if you want to disable stroking.
  pub fn set_stroke(&mut self, stroke: Option<Stroke>) {
    self.stroke = stroke;
  }

  /// Get the currently active stroke.
  pub fn get_stroke(&mut self) -> Option<&Stroke> {
    self.stroke.as_ref()
  }

  /// Draw a path using the currently active fill and/or stroke.
  pub fn draw_path(&mut self, path: &Path) {
    if self.fill.is_some() || self.stroke.is_some() {
      if self.has_complex_fill_or_stroke() {
        self.bd.get_mut().draw_path(
          &path.0,
          self.fill.as_ref(),
          None,
          self.sc,
          self.chunk_container,
        );
        self.bd.get_mut().draw_path(
          &path.0,
          None,
          self.stroke.as_ref(),
          self.sc,
          self.chunk_container,
        );
      } else {
        self.bd.get_mut().draw_path(
          &path.0,
          self.fill.as_ref(),
          self.stroke.as_ref(),
          self.sc,
          self.chunk_container,
        );
      }
    } else {
      // Draw with black by default.
      self.bd.get_mut().draw_path(
        &path.0,
        Some(&Fill::default()),
        None,
        self.sc,
        self.chunk_container,
      );
    }
  }

  /// Start a new tagged content section.
  ///
  /// # Panics
  /// Panics if a tagged section has already been started.
  /// Panics if an artifact with a required bbox has none.
  pub fn start_tagged(&mut self, tag: ContentTag) -> Identifier {
    if let Some(id) = &mut self.page_identifier {
      match tag {
        // An artifact is actually not really part of tagged PDF and doesn't have
        // a marked content identifier, so we need to return a dummy one here. It's just
        // the API of krilla that conflates artifacts with tagged content,
        // for the sake of simplicity. But the user of the library does not need to know
        // about this.
        ContentTag::Artifact(artifact) => {
          if matches!(artifact.kind, ArtifactType::Header | ArtifactType::Footer)
            && self.sc.serialize_settings().pdf_version()
              < VersionedFeature::HeaderFooterArtifactSubtypes.minimum_pdf_version()
          {
            self
              .sc
              .register_validation_error(ValidationError::RequiresNewerPdfVersion(
                VersionedFeature::HeaderFooterArtifactSubtypes,
                self.sc.location,
              ));
          }

          if artifact.requires_properties(self.sc.serialize_settings().pdf_version()) {
            self
              .bd
              .get_mut()
              .start_marked_content_with_properties(self.sc, None, tag);
          } else {
            self.bd.get_mut().start_marked_content(tag.name());
          }

          if artifact.requires_bbox(self.sc.serialize_settings().pdf_version())
            && artifact.bbox.is_none()
          {
            panic!("Background artifact must have a bounding box in PDF 1.7");
          }

          Identifier::dummy()
        }
        ContentTag::Span(_) | ContentTag::Other => {
          self
            .bd
            .get_mut()
            .start_marked_content_with_properties(self.sc, Some(id.mcid), tag);
          id.bump().into()
        }
      }
    } else {
      Identifier::dummy()
    }
  }

  /// End the current tagged section.
  ///
  /// # Panics
  /// Panics if no tagged section has been started.
  pub fn end_tagged(&mut self) {
    if self.page_identifier.is_some() {
      self.bd.get_mut().end_marked_content();
    }
  }

  fn outline_glyphs(
    &mut self,
    glyphs: &[impl Glyph],
    context_color: rgb::Color,
    start: Point,
    font: Font,
    font_size: f32,
  ) {
    let (mut cur_x, y) = (start.x, start.y);

    for glyph in glyphs {
      let mut base_transform = tiny_skia_path::Transform::from_translate(
        cur_x + glyph.x_offset(font_size),
        y - glyph.y_offset(font_size),
      );
      base_transform = base_transform.pre_concat(tiny_skia_path::Transform::from_scale(
        font_size / font.units_per_em(),
        -font_size / font.units_per_em(),
      ));
      draw_glyph(
        font.clone(),
        context_color,
        glyph.glyph_id(),
        Transform::from_tsp(base_transform),
        self,
      );

      cur_x += glyph.x_advance(font_size);
    }
  }

  /// Draw a sequence of glyphs using the currently active fill and/or stroke.
  ///
  /// This is a very low-level method, which gives you full control over how to place
  /// the glyphs that make up the text. This means that you must have your own text processing
  /// logic for dealing with bidirectional text, font fallback, text layouting, etc.
  pub fn draw_glyphs(
    &mut self,
    start: Point,
    glyphs: &[impl Glyph],
    font: Font,
    text: &str,
    font_size: f32,
    outlined: bool,
  ) {
    let context_color = self.context_color();
    if outlined {
      self.outline_glyphs(glyphs, context_color, start, font, font_size);
    } else {
      match (self.fill.as_ref(), self.stroke.as_ref()) {
        (Some(f), Some(s)) => {
          if self.has_complex_fill_or_stroke() {
            // We need to draw fill and stroke separately. In order to avoid embedding
            // text twice, we fill the text as normal text and then stroke it as a path.
            self.bd.get_mut().draw_glyphs(
              start,
              self.sc,
              self.chunk_container,
              Some(f),
              None,
              context_color,
              glyphs,
              font.clone(),
              text,
              font_size,
            );

            self.outline_glyphs(glyphs, context_color, start, font, font_size);
          } else {
            self.bd.get_mut().draw_glyphs(
              start,
              self.sc,
              self.chunk_container,
              Some(f),
              Some(s),
              context_color,
              glyphs,
              font,
              text,
              font_size,
            );
          }
        }
        (None, Some(s)) => {
          self.bd.get_mut().draw_glyphs(
            start,
            self.sc,
            self.chunk_container,
            None,
            Some(s),
            context_color,
            glyphs,
            font,
            text,
            font_size,
          );
        }
        (Some(f), None) => {
          self.bd.get_mut().draw_glyphs(
            start,
            self.sc,
            self.chunk_container,
            Some(f),
            None,
            context_color,
            glyphs,
            font,
            text,
            font_size,
          );
        }
        (None, None) => {
          // Draw with black by default.
          self.bd.get_mut().draw_glyphs(
            start,
            self.sc,
            self.chunk_container,
            Some(&Fill::default()),
            None,
            context_color,
            glyphs,
            font,
            text,
            font_size,
          );
        }
      }
    }
  }

  /// Set the location that should be assumed for subsequent operations.
  pub fn set_location(&mut self, location: Location) {
    self.sc.set_location(location);
  }

  /// Reset the location that should be assumed for subsequent operations.
  pub fn reset_location(&mut self) {
    self.sc.reset_location();
  }

  /// Return the current transformation matrix.
  pub fn ctm(&self) -> Transform {
    self.bd.get().cur_transform()
  }

  /// Concatenate a new transform to the current transformation matrix.
  pub fn push_transform(&mut self, transform: &Transform) {
    self.push_instructions.push(PushInstruction::Transform);
    self.bd.get_mut().save_graphics_state();
    self.bd.get_mut().concat_transform(transform);
  }

  /// Push a new blend mode.
  pub fn push_blend_mode(&mut self, blend_mode: BlendMode) {
    self.push_instructions.push(PushInstruction::BlendMode);
    self.bd.get_mut().save_graphics_state();
    self.bd.get_mut().set_blend_mode(blend_mode.to_pdf());
  }

  /// Push a new clip path.
  pub fn push_clip_path(&mut self, path: &Path, clip_rule: &FillRule) {
    self.push_instructions.push(PushInstruction::ClipPath);
    self.bd.get_mut().push_clip_path(&path.0, clip_rule);
  }

  /// Push a new mask.
  pub fn push_mask(&mut self, mask: Mask) {
    self
      .push_instructions
      .push(PushInstruction::Mask(Box::new(mask)));
    self
      .bd
      .sub_builders
      .push(ContentBuilder::new(Transform::identity(), true, self.sc));
  }

  /// Push a new opacity, meaning that each subsequent graphics object will be
  /// rendered with a base opacity.
  ///
  /// This stacks, meaning that if you do `push_opacity(0.5)` twice, the resulting
  /// base opacity will be 0.25.
  pub fn push_opacity(&mut self, opacity: NormalizedF32) {
    self
      .push_instructions
      .push(PushInstruction::Opacity(opacity));

    if opacity != NormalizedF32::ONE {
      self
        .bd
        .sub_builders
        .push(ContentBuilder::new(Transform::identity(), true, self.sc));
    }
  }

  /// Push a new isolated layer.
  pub fn push_isolated(&mut self) {
    self.push_instructions.push(PushInstruction::Isolated);
    self
      .bd
      .sub_builders
      .push(ContentBuilder::new(Transform::identity(), true, self.sc));
  }

  /// Pop the last `push` instruction.
  ///
  /// # Panics
  /// Panics if the there wasn't a corresponding `push` to the `pop`.
  pub fn pop(&mut self) {
    match self.push_instructions.pop().unwrap() {
      PushInstruction::Transform => self.bd.get_mut().restore_graphics_state(),
      PushInstruction::Opacity(o) => {
        if o != NormalizedF32::ONE {
          let stream = self.bd.sub_builders.pop().unwrap().finish(self.sc);
          self
            .bd
            .get_mut()
            .draw_opacified(self.sc, self.chunk_container, o, stream);
        }
      }
      PushInstruction::ClipPath => self.bd.get_mut().pop_clip_path(),
      PushInstruction::BlendMode => self.bd.get_mut().restore_graphics_state(),
      PushInstruction::Mask(mask) => {
        let stream = self.bd.sub_builders.pop().unwrap().finish(self.sc);
        self
          .bd
          .get_mut()
          .draw_masked(self.sc, self.chunk_container, *mask, stream)
      }
      PushInstruction::Isolated => {
        let stream = self.bd.sub_builders.pop().unwrap().finish(self.sc);
        self
          .bd
          .get_mut()
          .draw_isolated(self.sc, self.chunk_container, stream);
      }
    }
  }

  #[cfg(feature = "raster-images")]
  /// Draw a new bitmap image.
  pub fn draw_image(&mut self, image: Image, size: Size) {
    self
      .bd
      .get_mut()
      .draw_image(image, size, self.sc, self.chunk_container);
  }

  /// Draw a new graphic.
  ///
  /// Drawing the same graphic multiple times is very cheap in terms of
  /// file size.
  pub fn draw_graphic(&mut self, graphic: Graphic) {
    self.bd.get_mut().draw_xobject(
      self.sc,
      self.chunk_container,
      graphic.x_object,
      &ExtGState::new(),
    )
  }

  pub(crate) fn draw_shading(&mut self, shading: &ShadingFunction) {
    self
      .bd
      .get_mut()
      .draw_shading(shading, self.sc, self.chunk_container);
  }

  /// A convenience method for `std::mem::drop`.
  ///
  /// # Panics
  /// Panics if the push/pop difference is not 0.
  pub fn finish(self) {}

  pub(crate) fn draw_opacified_stream(&mut self, opacity: NormalizedF32, stream: Stream) {
    self
      .bd
      .get_mut()
      .draw_opacified(self.sc, self.chunk_container, opacity, stream)
  }

  /// Return the current transformation matrix of the surface.
  pub fn cur_transform(&self) -> Transform {
    self.bd.get().cur_transform()
  }

  fn context_color(&self) -> rgb::Color {
    self
      .fill
      .as_ref()
      .and_then(|f| f.paint.as_rgb())
      .or_else(|| self.stroke.as_ref().and_then(|s| s.paint.as_rgb()))
      .unwrap_or_default()
  }

  fn has_complex_fill_or_stroke(&self) -> bool {
    // Things can go wrong and yield inconsistent results in different PDF viewers
    // for some fill/stroke combinations. So in some cases, we need to fill and stroke
    // separately.
    // 1) We have an opacity on the stroke, because they render inconsistently across different
    // viewers.
    // 2) We have a linear gradient with opacities in at least one stop, because then we need
    // to invoke a soft mask, in which case the fill/stroke would both be affected by this.

    let check_paint = |paint: &Paint| match &paint.0 {
      InnerPaint::Color(_) => false,
      InnerPaint::LinearGradient(l) => l.stops.iter().any(|s| s.opacity != NormalizedF32::ONE),
      InnerPaint::RadialGradient(r) => r.stops.iter().any(|s| s.opacity != NormalizedF32::ONE),
      InnerPaint::SweepGradient(r) => r.stops.iter().any(|s| s.opacity != NormalizedF32::ONE),
      InnerPaint::Pattern(_) => false,
    };

    let complex_stroke = self
      .stroke
      .as_ref()
      .is_some_and(|s| s.opacity != NormalizedF32::ONE || check_paint(&s.paint));

    let complex_fill = self.fill.as_ref().is_some_and(|f| check_paint(&f.paint));

    complex_fill || complex_stroke
  }
}

impl Drop for Surface<'_> {
  fn drop(&mut self) {
    let root_builder = std::mem::replace(
      &mut self.bd.root_builder,
      ContentBuilder::new(Transform::identity(), false, self.sc),
    );
    let num_mcids = match self.page_identifier {
      Some(pi) => pi.mcid,
      None => 0,
    };

    assert!(self.bd.sub_builders.is_empty());
    assert!(self.push_instructions.is_empty());
    assert!(!root_builder.active_marked_content);

    (self.finish_fn)(root_builder.finish(self.sc), num_mcids)
  }
}

/// Holds the different content streams we are currently building. In the usual case,
/// this only contains the current page stream as the root builder, but the sub builders
/// will be used if we are for example creating a mask/pattern, or an XObject.
struct Builders {
  pub(crate) root_builder: ContentBuilder,
  pub(crate) sub_builders: Vec<ContentBuilder>,
}

impl Builders {
  fn new(root_builder: ContentBuilder) -> Self {
    Self {
      root_builder,
      sub_builders: vec![],
    }
  }

  fn get_mut(&mut self) -> &mut ContentBuilder {
    self
      .sub_builders
      .last_mut()
      .unwrap_or(&mut self.root_builder)
  }

  fn get(&self) -> &ContentBuilder {
    self.sub_builders.last().unwrap_or(&self.root_builder)
  }
}

pub(crate) enum PushInstruction {
  Transform,
  Opacity(NormalizedF32),
  ClipPath,
  BlendMode,
  Mask(Box<Mask>),
  Isolated,
}
