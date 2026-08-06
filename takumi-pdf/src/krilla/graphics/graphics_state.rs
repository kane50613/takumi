use std::hash::Hash;
use std::sync::Arc;

use pdf_writer::types::BlendMode;
use pdf_writer::{Finish, Name, Ref};

use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::configure::ValidationError;
use crate::krilla::geom::{Rect, Transform};
use crate::krilla::graphics::mask::Mask;
use crate::krilla::num::NormalizedF32;
use crate::krilla::resource;
use crate::krilla::resource::Resourceable;
use crate::krilla::serialize::{Cacheable, SerializeContext};

/// The inner representation of an external graphics state.
#[derive(Debug, Hash, PartialEq, Eq, Default, Clone)]
struct Repr {
  /// The non-stroking alpha.
  non_stroking_alpha: Option<NormalizedF32>,
  /// The stroking alpha.
  stroking_alpha: Option<NormalizedF32>,
  /// The blend mode.
  blend_mode: Option<BlendMode>,
  /// An active mask.
  mask: Option<Ref>,
}

/// A graphics state containing information about
/// - The current stroking alpha.
/// - The current non-stroking alpha.
/// - The current blend mode.
/// - The current mask.
///
/// This struct provides exposes a builder pattern for setting the various properties
/// individually.
///
/// This type is cheap to clone.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Default)]
pub(crate) struct ExtGState(Arc<Repr>);

impl ExtGState {
  /// Create a new, empty graphics state.
  pub(crate) fn new() -> Self {
    Self::default()
  }

  /// Create a new graphics state with a stroking alpha.
  #[must_use]
  pub(crate) fn stroking_alpha(mut self, stroking_alpha: NormalizedF32) -> Self {
    Arc::make_mut(&mut self.0).stroking_alpha = Some(stroking_alpha);
    self
  }

  /// Create a new graphics state with a non-stroking alpha.
  #[must_use]
  pub(crate) fn non_stroking_alpha(mut self, non_stroking_alpha: NormalizedF32) -> Self {
    Arc::make_mut(&mut self.0).non_stroking_alpha = Some(non_stroking_alpha);
    self
  }

  /// Create a new graphics state with a blend mode.
  #[must_use]
  pub(crate) fn blend_mode(mut self, blend_mode: BlendMode) -> Self {
    Arc::make_mut(&mut self.0).blend_mode = Some(blend_mode);
    self
  }

  /// Create a new graphics state with a mask.
  #[must_use]
  pub(crate) fn mask(
    mut self,
    mask: Mask,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
  ) -> Self {
    let mask_ref = sc.register_cacheable(chunk_container, mask);
    Arc::make_mut(&mut self.0).mask = Some(mask_ref);
    self
  }

  /// Check whether the graphics state is empty.
  pub(crate) fn empty(&self) -> bool {
    self.0.mask.is_none()
      && self.0.stroking_alpha.is_none()
      && self.0.non_stroking_alpha.is_none()
      && self.0.blend_mode.is_none()
  }

  /// Integrate another graphics state into the current one. This is done by replacing
  /// all active properties of the other graphics state in the current graphics state, while
  /// leaving the inactive ones unchanged.
  pub(crate) fn combine(&mut self, other: &ExtGState) {
    if let Some(stroking_alpha) = other.0.stroking_alpha {
      Arc::make_mut(&mut self.0).stroking_alpha = Some(stroking_alpha);
    }

    if let Some(non_stroking_alpha) = other.0.non_stroking_alpha {
      Arc::make_mut(&mut self.0).non_stroking_alpha = Some(non_stroking_alpha);
    }

    if let Some(blend_mode) = other.0.blend_mode {
      Arc::make_mut(&mut self.0).blend_mode = Some(blend_mode);
    }

    if let Some(mask) = other.0.mask {
      Arc::make_mut(&mut self.0).mask = Some(mask);
    }
  }
}

impl Cacheable for ExtGState {
  fn serialize(
    self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    let chunk = &mut chunk_container.non_stream.ext_g_states;

    let mut ext_st = chunk.ext_graphics(root_ref);
    if let Some(nsa) = self.0.non_stroking_alpha {
      if nsa != NormalizedF32::ONE {
        sc.register_validation_error(ValidationError::Transparency(sc.location));
      }

      ext_st.non_stroking_alpha(nsa.get());
    }

    if let Some(sa) = self.0.stroking_alpha {
      if sa != NormalizedF32::ONE {
        sc.register_validation_error(ValidationError::Transparency(sc.location));
      }

      ext_st.stroking_alpha(sa.get());
    }

    if let Some(bm) = self.0.blend_mode {
      if bm != BlendMode::Normal {
        sc.register_validation_error(ValidationError::Transparency(sc.location));
      }

      ext_st.blend_mode(bm);
    }

    if let Some(mask_ref) = self.0.mask {
      sc.register_validation_error(ValidationError::Transparency(sc.location));

      ext_st.pair(Name(b"SMask"), mask_ref);
    }

    ext_st.finish();
  }
}

impl Resourceable for ExtGState {
  type Resource = resource::ExtGState;
}

/// A simulation of the PDF graphics state, so that we
/// can write our transforms/graphics state all at once
/// when adding an image/path instead of having to
/// use `save_state`/`restore_state` excessively.
#[derive(Debug, PartialEq, Clone)]
pub(crate) struct GraphicsState {
  ext_g_state: ExtGState,
  ctm: Transform,
}
impl Eq for GraphicsState {}

impl Hash for GraphicsState {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.ext_g_state.hash(state);
    self.ctm.hash(state);
  }
}

impl Default for GraphicsState {
  fn default() -> Self {
    Self {
      ext_g_state: ExtGState::new(),
      ctm: Transform::identity(),
    }
  }
}

impl GraphicsState {
  pub(crate) fn combine(&mut self, other: &ExtGState) {
    self.ext_g_state.combine(other);
  }

  pub(crate) fn concat_transform(&mut self, transform: Transform) {
    self.ctm = self.ctm.pre_concat(transform);
  }

  pub(crate) fn transform(&self) -> Transform {
    self.ctm
  }

  pub(crate) fn ext_g_state(&self) -> &ExtGState {
    &self.ext_g_state
  }
}

/// A collection of graphics states, simulates the stack-like
/// structure used.
#[derive(Debug, Hash, Eq, PartialEq)]
pub struct GraphicsStates {
  graphics_states: Vec<GraphicsState>,
}

impl GraphicsStates {
  pub(crate) fn new() -> Self {
    GraphicsStates {
      graphics_states: vec![GraphicsState::default()],
    }
  }

  pub(crate) fn cur(&self) -> &GraphicsState {
    self.graphics_states.last().unwrap()
  }

  pub(crate) fn cur_mut(&mut self) -> &mut GraphicsState {
    self.graphics_states.last_mut().unwrap()
  }

  pub(crate) fn save_state(&mut self) {
    let state = self.cur();
    self.graphics_states.push(state.clone())
  }

  pub(crate) fn restore_state(&mut self) {
    self.graphics_states.pop();
  }

  pub(crate) fn combine(&mut self, other: &ExtGState) {
    self.cur_mut().combine(other);
  }

  pub(crate) fn transform(&mut self, transform: Transform) {
    self.cur_mut().concat_transform(transform);
  }

  pub(crate) fn transform_bbox(&self, bbox: Rect) -> Rect {
    // Important: This does not take the root transform of the
    // corresponding ContentBuilder into account, because we
    // want it to be in krilla coordinates, not in PDF
    // coordinates.
    bbox.transform(self.cur().transform()).unwrap()
  }
}
