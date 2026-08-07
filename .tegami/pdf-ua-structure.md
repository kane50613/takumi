---
packages:
  takumi-pdf:
    type: patch
---

### Emit a structure tree PDF/UA accepts

Headings are renumbered by nesting depth, so a document that opens at `h2` or jumps from `h1` to `h4` no longer writes a tree the validator rejects. A list item outside a list now brings its own list. A heading whose text sits in child elements, such as `<h1>Plain <strong>bold</strong></h1>`, reaches the outline instead of being dropped, which used to fail a `tagged: "ua1"` render outright.
