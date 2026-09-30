---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Collapse white space as Chrome does

- `&nbsp;`, U+3000 and other spaces outside CSS's document white space keep their width under `white-space-collapse: collapse`.
