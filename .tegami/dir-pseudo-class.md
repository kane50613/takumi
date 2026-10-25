---
packages:
  takumi-core: minor
---

# Support `:dir()`

The `:dir(ltr)` and `:dir(rtl)` selectors now match an element by its directionality, taken from the `dir` attribute on the element or its nearest ancestor, and `ltr` when none sets one.
