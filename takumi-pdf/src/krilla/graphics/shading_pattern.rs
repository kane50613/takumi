use std::hash::Hash;
use std::sync::Arc;

use pdf_writer::{Finish, Name, Ref};

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::geom::Transform;
use crate::krilla::graphics::shading_function::{GradientProperties, ShadingFunction};
use crate::krilla::resource;
use crate::krilla::resource::Resourceable;
use crate::krilla::serialize::{Cacheable, SerializeContext};

#[derive(Debug, PartialEq)]
struct Repr {
  shading_function: ShadingFunction,
  shading_transform: Transform,
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
  pub fn new(gradient_properties: GradientProperties, shading_transform: Transform) -> Self {
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
    shading_pattern.matrix(self.0.shading_transform.to_pdf_transform());

    shading_pattern.finish();
  }
}

impl Resourceable for ShadingPattern {
  type Resource = resource::Pattern;
}
