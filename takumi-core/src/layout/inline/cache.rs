use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use super::ShapedRun;

/// The faces the runs of a render drew in lately, which the next runs drawing in an equal one
/// share.
#[derive(Clone, Default)]
pub(crate) struct InlineLayoutCache {
  faces: Rc<RefCell<VecDeque<Rc<ShapedRun>>>>,
}

impl InlineLayoutCache {
  /// `face`, or an equal face a run laid out lately drew in.
  pub(crate) fn share_face(&self, face: ShapedRun) -> Rc<ShapedRun> {
    const RECENT_FACES: usize = 8;

    let mut faces = self.faces.borrow_mut();

    if let Some(shared) = faces.iter().find(|shared| shared.same_face(&face)) {
      return Rc::clone(shared);
    }

    let face = Rc::new(face);

    if faces.len() == RECENT_FACES {
      faces.pop_front();
    }

    faces.push_back(Rc::clone(&face));
    face
  }
}
