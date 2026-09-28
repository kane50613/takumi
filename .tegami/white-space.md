---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Collapse white space as Chrome does

- `&nbsp;`, U+3000 and other spaces outside CSS's document white space keep their width under `white-space-collapse: collapse`.
- Collapsible spaces at the start and end of a paragraph drop out, such as indented HTML or `<span>Label </span>` inside a flex row.
