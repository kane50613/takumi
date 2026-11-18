---
packages:
  takumi-core: minor
---

# `FromHtmlOptions` can parse a fragment as a whole document

`FromHtmlOptions::builder().document(true)` parses the source as a browser opens an HTML file, with `html` and `body` elements, even when it starts with neither a doctype nor `<html>`. Rules such as `body { font-family: … }` then match. Without it, such a source still parses as a fragment.
