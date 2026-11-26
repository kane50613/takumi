//! A persistent hash map, as Blink's `HashTrieNode` stores `StyleVariables`: a copy shares every
//! node, and a change copies only the nodes on its key's path.

use std::{
  borrow::Borrow,
  hash::{BuildHasher, Hash},
  sync::Arc,
};

use rustc_hash::FxBuildHasher;

/// The hash bits each level of the trie consumes.
const BITS: u32 = 4;
const WIDTH: usize = 1 << BITS;

/// A map from `K` to `V` whose clones share their nodes.
#[derive(Debug, Clone)]
pub(crate) struct HashTrie<K, V> {
  root: Option<Arc<Node<K, V>>>,
  len: usize,
}

#[derive(Debug, Clone)]
enum Node<K, V> {
  Branch([Option<Arc<Node<K, V>>>; WIDTH]),
  /// The entries whose keys hash alike, usually one.
  Leaf(u64, Vec<(K, V)>),
}

impl<K, V> Default for HashTrie<K, V> {
  fn default() -> Self {
    Self { root: None, len: 0 }
  }
}

fn hash<Q: Hash + ?Sized>(key: &Q) -> u64 {
  FxBuildHasher.hash_one(key)
}

/// The slot `hash` takes at `depth`.
fn slot(hash: u64, depth: u32) -> usize {
  ((hash >> (depth * BITS)) as usize) & (WIDTH - 1)
}

impl<K: Clone + Eq + Hash, V: Clone> HashTrie<K, V> {
  pub(crate) fn get<Q>(&self, key: &Q) -> Option<&V>
  where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
  {
    let hash = hash(key);
    let mut node = self.root.as_deref()?;
    let mut depth = 0;

    loop {
      match node {
        Node::Branch(slots) => {
          node = slots[slot(hash, depth)].as_deref()?;
          depth += 1;
        }
        Node::Leaf(leaf_hash, entries) => {
          return (*leaf_hash == hash)
            .then(|| entries.iter().find(|(entry, _)| entry.borrow() == key))
            .flatten()
            .map(|(_, value)| value);
        }
      }
    }
  }

  pub(crate) fn insert(&mut self, key: K, value: V) {
    let hash = hash(&key);

    if insert(&mut self.root, hash, key, value, 0) {
      self.len += 1;
    }
  }

  /// Removes `key`, returning its value.
  pub(crate) fn remove<Q>(&mut self, key: &Q) -> Option<V>
  where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
  {
    let removed = remove(&mut self.root, hash(key), key, 0)?;

    self.len -= 1;
    Some(removed)
  }

  /// Every entry, in no particular order.
  pub(crate) fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
    let mut pending: Vec<&Node<K, V>> = self.root.as_deref().into_iter().collect();
    let mut entries: std::slice::Iter<'_, (K, V)> = [].iter();

    std::iter::from_fn(move || {
      loop {
        if let Some((key, value)) = entries.next() {
          return Some((key, value));
        }
        match pending.pop()? {
          Node::Branch(slots) => pending.extend(slots.iter().flatten().map(Arc::as_ref)),
          Node::Leaf(_, leaf) => entries = leaf.iter(),
        }
      }
    })
  }
}

/// Inserts into the subtrie at `node`, copying the nodes another trie shares on the way, and
/// returns whether `key` is new.
fn insert<K: Clone + Eq, V: Clone>(
  node: &mut Option<Arc<Node<K, V>>>,
  hash: u64,
  key: K,
  value: V,
  depth: u32,
) -> bool {
  let Some(node) = node else {
    *node = Some(Arc::new(Node::Leaf(hash, vec![(key, value)])));
    return true;
  };

  let node = Arc::make_mut(node);

  match node {
    Node::Branch(slots) => insert(&mut slots[slot(hash, depth)], hash, key, value, depth + 1),
    Node::Leaf(leaf_hash, entries) if *leaf_hash == hash => {
      if let Some(entry) = entries.iter_mut().find(|(entry, _)| *entry == key) {
        entry.1 = value;
        return false;
      }
      entries.push((key, value));
      true
    }
    // Another hash: the leaf moves a level down, under a branch both keys share.
    Node::Leaf(leaf_hash, entries) => {
      let mut slots: [Option<Arc<Node<K, V>>>; WIDTH] = Default::default();

      slots[slot(*leaf_hash, depth)] =
        Some(Arc::new(Node::Leaf(*leaf_hash, std::mem::take(entries))));

      let added = insert(&mut slots[slot(hash, depth)], hash, key, value, depth + 1);

      *node = Node::Branch(slots);
      added
    }
  }
}

/// Removes `key` from the subtrie at `node`, dropping a node it leaves empty.
fn remove<K, V, Q>(node: &mut Option<Arc<Node<K, V>>>, hash: u64, key: &Q, depth: u32) -> Option<V>
where
  K: Clone + Borrow<Q>,
  V: Clone,
  Q: Eq + ?Sized,
{
  let shared = node.as_ref()?;

  // Look before copying, so that a key not here copies nothing.
  if let Node::Leaf(leaf_hash, entries) = shared.as_ref()
    && (*leaf_hash != hash || !entries.iter().any(|(entry, _)| entry.borrow() == key))
  {
    return None;
  }

  let removed = match Arc::make_mut(node.as_mut()?) {
    Node::Branch(slots) => {
      let removed = remove(&mut slots[slot(hash, depth)], hash, key, depth + 1)?;

      if slots.iter().all(Option::is_none) {
        *node = None;
      }
      return Some(removed);
    }
    Node::Leaf(_, entries) => {
      let index = entries
        .iter()
        .position(|(entry, _)| entry.borrow() == key)?;

      entries.swap_remove(index).1
    }
  };

  if matches!(node.as_deref(), Some(Node::Leaf(_, entries)) if entries.is_empty()) {
    *node = None;
  }
  Some(removed)
}

impl<K: Clone + Eq + Hash, V: Clone + PartialEq> PartialEq for HashTrie<K, V> {
  fn eq(&self, other: &Self) -> bool {
    let shared = match (&self.root, &other.root) {
      (Some(left), Some(right)) => Arc::ptr_eq(left, right),
      (None, None) => true,
      _ => false,
    };

    shared
      || (self.len == other.len
        && self
          .iter()
          .all(|(key, value)| other.get(key) == Some(value)))
  }
}

#[cfg(test)]
mod tests {
  use super::HashTrie;

  #[test]
  fn a_clone_keeps_its_entries_when_the_original_changes() {
    let mut trie = HashTrie::default();

    for index in 0..200 {
      trie.insert(format!("--v{index}"), index.to_string());
    }

    let snapshot = trie.clone();

    trie.insert("--v7".to_owned(), "changed".to_owned());
    trie.insert("--new".to_owned(), "new".to_owned());
    assert_eq!(trie.remove("--v8"), Some("8".to_owned()));
    assert_eq!(trie.remove("--missing"), None);

    assert_eq!(snapshot.get("--v7").map(String::as_str), Some("7"));
    assert_eq!(snapshot.get("--v8").map(String::as_str), Some("8"));
    assert_eq!(snapshot.get("--new"), None);
    assert_eq!(trie.get("--v7").map(String::as_str), Some("changed"));
    assert_eq!(trie.get("--v8"), None);
    assert_eq!(trie.iter().count(), 200);
    assert_eq!(snapshot.iter().count(), 200);
    assert_ne!(trie, snapshot);
  }

  #[test]
  fn tries_with_the_same_entries_are_equal() {
    let mut left = HashTrie::default();
    let mut right = HashTrie::default();

    for index in 0..50 {
      left.insert(index, index * 2);
    }
    for index in (0..50).rev() {
      right.insert(index, index * 2);
    }
    assert_eq!(left, right);

    for index in 0..50 {
      left.remove(&index);
    }
    assert_eq!(left, HashTrie::default());
  }
}
