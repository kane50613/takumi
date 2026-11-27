use std::hash::Hash;
use std::sync::Arc;
use takumi_core::style::Affine;

use pdf_writer::{Finish, Name, Ref};

use crate::writer::chunk_container::ChunkContainer;
use crate::writer::graphics::shading_function::{GradientProperties, ShadingFunction};
use crate::writer::resource;
use crate::writer::resource::Resourceable;
use crate::writer::serialize::{Cacheable, SerializeContext};

#[derive(Debug, PartialEq)]
struct Repr {
  shading_function: ShadingFunction,
  shading_transform: Affine,
}

impl Eq for Repr {}

impl Hash for Repr {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.shading_function.hash(state);
    self.shading_transform.hash(state);
  }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub(crate) struct ShadingPattern(Arc<Repr>);

impl ShadingPattern {
  pub fn new(gradient_properties: GradientProperties, shading_transform: Affine) -> Self {
    Self(Arc::new(Repr {
      // CTM doesn't need to be included to calculate the domain of the shading function
      shading_function: ShadingFunction::new(gradient_properties, false),
      shading_transform,
    }))
  }
}

impl Cacheable for ShadingPattern {
  fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    let shading_ref = sc.register_cacheable(chunk_container, self.0.shading_function.clone());
    let chunk = &mut chunk_container.non_stream.patterns;
    let mut shading_pattern = chunk.shading_pattern(root_ref);
    shading_pattern.pair(Name(b"Shading"), shading_ref);
    shading_pattern.matrix(self.0.shading_transform.to_cols_array());

    shading_pattern.finish();
  }
}

impl Resourceable for ShadingPattern {
  type Resource = resource::Pattern;
}
