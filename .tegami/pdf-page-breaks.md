---
packages:
  "takumi-core": minor
  "takumi-pdf": patch
---

# Break pages as Chrome prints them

- A page break falls between line boxes. A paragraph taller than a page no longer leaves the first page blank, and short paragraphs can split too.
- A line or image taller than a page moves to the next page, then continues over the pages after it.
- A box's top and bottom border and padding stay with its first and last line.
- A table row whose cell padding does not fit moves to the next page whole.
