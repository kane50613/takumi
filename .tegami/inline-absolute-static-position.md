---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Place absolute boxes inside text where Chrome places them

An absolutely positioned box inside a paragraph now keeps the text around it on one line and starts where it sits in that line, as Chrome does. Before, it split the paragraph in two and could land at the paragraph's top or inside a line's text.
