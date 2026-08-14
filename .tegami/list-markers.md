---
packages:
  takumi-core:
    type: minor
  takumi-html:
    type: minor
  "@takumi-rs/helpers":
    type: minor
---

### Draw list markers for `<ol>` and `<ul>`

List items rendered with no bullet or number. A `display: list-item` box now generates a marker: `list-style-type`, `list-style-position` and `list-style-image` pick what it draws and where it sits, and `<ol start>` and `<li value>` set the count.
