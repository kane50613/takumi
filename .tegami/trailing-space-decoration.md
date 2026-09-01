---
packages:
  "takumi-core":
    type: patch
---

### Stop decorating line-end whitespace

Underline, overline, and line-through no longer extend over the collapsed
whitespace at a line break, matching browsers for LTR text. RTL is
approximate: the trim lands on the visual right edge.
