---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# Content after a line taller than a page starts below it

When a line held something taller than a page, such as a tall `inline-block`, the lines after it started at the top of the next page and drew over the part that ran on. They now start below it, as Chromium prints them.
