---
packages:
  "takumi-pdf": patch
---

# Run a line taller than a page on over the pages after it

In paged PDF output, a line, image or other unsplittable box taller than a page now moves to the next page when content precedes it, then continues over the pages after it, as Chrome prints it. Before, the page cut through it where it fell, and a tall line drew only on the page holding its baseline, losing the rest.
