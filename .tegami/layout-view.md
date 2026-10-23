---
packages:
  takumi-core: minor
---

# Read a layout while it still lays out pages

`LayoutView` answers the same questions as `LayoutResults` for a finished layout or for a `PagedLayout` that has laid out some pages. `PagedLayout::finalized_pages` counts the pages no later page can paint onto.
