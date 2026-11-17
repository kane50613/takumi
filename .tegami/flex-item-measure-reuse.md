---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# A long table in a column flexbox takes less memory to render

A PDF in which a column flexbox breaks across many pages takes less memory to render, and the PDF stays the same. An 11,000-row table in a column flexbox peaks at 1.35 GB in place of 1.46 GB.
