---
packages:
  "takumi": patch
---

# Skip text in `:first-child` and sibling selectors

`:first-child`, `:last-child`, `:nth-child()`, `+` and `~` skip text nodes, as browsers do. `td:first-child` now matches in indented HTML.
