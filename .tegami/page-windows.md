---
packages:
  takumi-core: minor
---

# Build the scene of one page from the pages laid out so far

`SceneRef::window` builds the scene of some of a paged layout's root fragments from a `LayoutView`, and `PageWindows` says which pages a page needs. `LayoutView` reads only the fragments a `PagedLayout` has placed.
