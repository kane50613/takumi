//! Tiling patterns.

use std::hash::{Hash, Hasher};
use std::ops::DerefMut;

use pdf_writer::types::{PaintType, TilingType};
use pdf_writer::{Finish, Ref};

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::geom::Transform;
use crate::krilla::num::NormalizedF32;
use crate::krilla::resource;
use crate::krilla::resource::Resourceable;
use crate::krilla::serialize::{Cacheable, SerializeContext};
use crate::krilla::stream::StreamBuilder;
use crate::krilla::stream::{FilterStreamBuilder, Stream};

#[derive(Debug, PartialEq)]
pub(crate) struct TilingPattern {
  stream: Stream,
  transform: Transform,
  base_opacity: NormalizedF32,
  width: f32,
  height: f32,
}

impl Eq for TilingPattern {}

impl Hash for TilingPattern {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.stream.hash(state);
    self.transform.hash(state);
    self.base_opacity.hash(state);
    self.width.to_bits().hash(state);
    self.height.to_bits().hash(state);
  }
}

impl TilingPattern {
  pub(crate) fn new(
    stream: Stream,
    transform: Transform,
    base_opacity: NormalizedF32,
    width: f32,
    height: f32,
    serializer_context: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
  ) -> Self {
    // stroke/fill opacity doesn't work consistently across different viewers for patterns,
    // so instead we simulate it ourselves.
    let pattern_stream = if base_opacity == NormalizedF32::ONE {
      stream
    } else {
      let stream = {
        let mut builder = StreamBuilder::new(serializer_context, chunk_container);
        let mut surface = builder.surface();
        surface.draw_opacified_stream(base_opacity, stream);
        surface.finish();
        builder.finish()
      };

      stream
    };

    Self {
      stream: pattern_stream,
      transform,
      base_opacity,
      width,
      height,
    }
  }
}

impl Cacheable for TilingPattern {
  fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    let mut chunk = sc.new_chunk();

    for validation_error in self.stream.validation_errors {
      sc.register_validation_error(validation_error);
    }

    let pattern_stream =
      FilterStreamBuilder::new_from_content_stream(&self.stream.content, &sc.serialize_settings())
        .finish(&sc.serialize_settings());
    let mut tiling_pattern = chunk.tiling_pattern(root_ref, pattern_stream.encoded_data());
    pattern_stream.write_filters(tiling_pattern.deref_mut().deref_mut());

    self.stream.resource_dictionary.to_pdf_resources(
      &mut tiling_pattern,
      sc,
      &mut chunk_container.non_stream.resource_dictionaries,
    );

    let final_bbox = pdf_writer::Rect::new(0.0, 0.0, self.width, self.height);

    tiling_pattern
      .tiling_type(TilingType::ConstantSpacing)
      .paint_type(PaintType::Colored)
      .bbox(final_bbox)
      .matrix(self.transform.to_pdf_transform())
      .x_step(final_bbox.x2 - final_bbox.x1)
      .y_step(final_bbox.y2 - final_bbox.y1);

    tiling_pattern.finish();
    chunk_container.streams.patterns.push(chunk);
  }
}

impl Resourceable for TilingPattern {
  type Resource = resource::Pattern;
}
