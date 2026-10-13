---
packages:
  "takumi-html": patch
  "takumi": patch
---

# Parse repeated inline styles once

`from_html` parses each distinct `style` attribute once per document and reuses it for every element that repeats it. A dashboard of 48 cards with shared styles builds in half the time.
