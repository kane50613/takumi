---
packages:
  takumi-core: patch
---

# Match fonts once per style in a render

Each run of inline text matched its fonts and fallback faces again, even when an earlier run had the same font style. Runs with the same font style now share one fallback list for the whole render.
