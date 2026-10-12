---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Collapse white space as Chrome does

- `&nbsp;`, U+3000 and other non-CSS spaces keep their width.
- Spaces at the start and end of a paragraph drop out, as in `<span>Label </span>` inside a flex row.
- A float at the start of a line no longer keeps the space after it.
- `<br>` always starts a new line.
- Under `pre-wrap`, `"A \nB"` renders as two lines.
- A mixed-direction line hangs its trailing space past the edge.
