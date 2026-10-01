---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Collapse white space as Chrome does

- `&nbsp;`, U+3000 and other spaces outside CSS's document white space keep their width under `white-space-collapse: collapse`.
- Collapsible spaces at the start and end of a paragraph drop out, such as indented HTML or `<span>Label </span>` inside a flex row.
- A float at the start of a line no longer keeps the space after it.
- `<br>` always starts a new line, even with the style presets off or `white-space` set to collapse newlines.
- A right-to-left line that ends in left-to-right words, or the reverse, hangs its line-end space past the edge and leaves it out of decorations and backgrounds.
- Under `white-space: pre-wrap`, a newline right after a space ends the line. `"A \nB"` used to render as one line.
