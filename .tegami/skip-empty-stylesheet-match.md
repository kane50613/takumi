---
packages:
  "cargo:takumi-core": patch
---

### Skip the style-match arena allocations when no CSS rules apply

`match_stylesheets_view` now filters the stylesheet rules before it builds the per-node match buckets, and returns early when nothing survives. Renders driven only by inline styles or Tailwind classes (the common case) no longer allocate the per-node bucket vectors or walk the matcher for zero rules. Rendered output is unchanged.
