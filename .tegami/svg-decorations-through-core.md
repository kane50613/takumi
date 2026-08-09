---
packages:
  takumi-svg:
    type: patch
---

### Emit a text decoration as one rect

An underline, overline or line-through used to be a `<rect>` wrapped in a `<g transform>`. It is now a single positioned `<rect>`, so a document full of decorated text is smaller. A decoration with no width, no height or a fully transparent colour no longer reaches the output.
