use std::sync::Arc;

use smallvec::SmallVec;

/// A task for resolving a resource URL.
pub type FetchTask = Arc<str>;

/// Collection of unique fetch tasks.
#[derive(Default)]
#[non_exhaustive]
pub struct FetchTaskCollection(SmallVec<[FetchTask; 8]>);

impl FetchTaskCollection {
  /// Insert a new task.
  pub fn insert(&mut self, task: FetchTask) {
    if !self.0.contains(&task) {
      self.0.push(task);
    }
  }

  /// Bulk insert tasks.
  pub fn insert_many(&mut self, tasks: impl IntoIterator<Item = FetchTask>) {
    let tasks_iter = tasks.into_iter();

    for task in tasks_iter {
      self.insert(task);
    }
  }

  /// Consumes the collection and returns the underlying `SmallVec`.
  pub fn into_inner(self) -> SmallVec<[FetchTask; 8]> {
    self.0
  }
}
