---
packages:
  takumi-core: minor
---

# `FromHtmlOptions` can parse a fragment as a whole document

The `document` option of `FromHtmlOptions` parses the source as a browser opens an HTML file, with `html` and `body` elements, even when it starts with neither a doctype nor `<html>`. Rules such as `body { font-family: … }` then match. Without it, such a source still parses as a fragment.

```rust
let options = FromHtmlOptions::builder().document(true).build();
// The tree has `html`, `head`, and `body`, so the `body` rule matches
let parsed = parse_html("<style>body { margin: 0 }</style><p>Hello</p>", options)?;
let stylesheet = StyleSheet::parse(&parsed.css.join("\n"))?;
// Render `parsed.node` with `.stylesheet(stylesheet.into())`
```
