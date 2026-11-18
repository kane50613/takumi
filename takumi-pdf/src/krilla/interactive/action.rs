//! PDF actions, which a link annotation performs when activated.

use pdf_writer::Str;
use pdf_writer::types::ActionType;

/// A link action. Will open a link when clicked.
pub struct LinkAction {
  uri: String,
}

impl LinkAction {
  /// Create a new link action that will open a URI when clicked.
  pub fn new(uri: String) -> Self {
    Self { uri }
  }

  pub(crate) fn serialize(&self, mut action: pdf_writer::writers::Action) {
    action
      .action_type(ActionType::Uri)
      .uri(Str(self.uri.as_bytes()));
  }
}
