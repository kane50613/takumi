---
packages:
  takumi-core: patch
---

# Lay out inline text as Chrome does

Text and inline boxes now lay out with a port of Chromium's inline layout. `text-overflow: ellipsis` and `line-clamp` cut at a character instead of a whole word, a tab advances to the next tab stop, a float that no longer fits beside a line moves below it, and `text-wrap: balance` leaves a paragraph with floats unbalanced, as Chrome does.
