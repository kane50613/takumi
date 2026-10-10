---
packages:
  "takumi": patch
---

# Match large stylesheets faster

A stylesheet's rules are indexed once when it is parsed, not on every render. A render visits only the rules for the ids, classes and tags its nodes use, so passing a whole site's CSS no longer costs time for every rule in it.
