---
packages:
  "takumi": patch
---

# Keep `text-indent` on a `text-fit` line

- A `text-fit` line keeps its `text-indent`, and `center`, `right` and right-to-left lines land where Chrome puts them. A grown first line used to lose part of its indent.
- A `per-line` or `per-line-all` fit counts the indent when deciding whether a line is already within 2px of its box, as Chrome does.
