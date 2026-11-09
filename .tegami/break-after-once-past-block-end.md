---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# A forced break after a box with an absolute box running on applies once

When an absolutely positioned box ran on past the page from inside a box with `break-after: page`, the forced break applied again on the next page, so a float after the box landed a page later than in Chromium. It now applies once, where the box's content ends.
