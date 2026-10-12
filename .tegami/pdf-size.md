---
packages:
  "takumi-pdf": patch
---

# Write smaller PDFs

- Each page writes only the clips and groups it shows. Documents with `overflow: hidden` or `opacity` come out smaller, and long ones render faster.
- A tagged PDF with more than 65,535 structure elements compresses its structure tree. An 11,000-row table is 3.5 times smaller.
