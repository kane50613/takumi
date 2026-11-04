---
packages:
  takumi-core: patch
---

# `:has()` matches

A `:has()` selector used to parse as an unknown pseudo-class and never match, so `:not(:has(…))` always matched. It now matches its relative selectors, `>`, `+` and `~` included, as browsers do.
