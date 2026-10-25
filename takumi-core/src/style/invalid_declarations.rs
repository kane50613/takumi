use std::cell::RefCell;

thread_local! {
  static COLLECTING: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// The declarations style objects dropped for an invalid value, each described as an error message.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InvalidDeclarations(Vec<String>);

impl InvalidDeclarations {
  /// Runs `read`, collecting the declarations dropped by the style objects it deserializes on this thread.
  pub fn collect<T>(read: impl FnOnce() -> T) -> (T, Self) {
    let outer = Restore(COLLECTING.replace(Some(Vec::new())));
    let value = read();
    let messages = COLLECTING.take().unwrap_or_default();

    drop(outer);

    (value, Self(messages))
  }

  pub(crate) fn record(message: impl FnOnce() -> String) {
    COLLECTING.with_borrow_mut(|collecting| {
      if let Some(messages) = collecting {
        messages.push(message());
      }
    });
  }

  /// The error messages, in the order the declarations were read.
  pub fn messages(&self) -> &[String] {
    &self.0
  }

  /// Whether every declaration was valid.
  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}

/// Puts the enclosing collection back, even when `read` unwinds.
struct Restore(Option<Vec<String>>);

impl Drop for Restore {
  fn drop(&mut self) {
    COLLECTING.set(self.0.take());
  }
}
