---
packages:
  "takumi": patch
---

# Skip text between elements in `:first-child` and sibling selectors

`:first-child`, `:last-child`, `:nth-child()` and the `+` and `~` combinators now skip text nodes, as browsers do. Before, the whitespace between tags in HTML counted as a sibling, so `td:first-child` matched nothing.
