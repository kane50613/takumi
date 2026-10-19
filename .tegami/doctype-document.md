---
packages:
  takumi-html: minor
---

# Parse a source with a doctype as a document

`from_html` parses a source that opens with a doctype as a document even without an `<html>` tag, so it gains the `<html>`, `<head>`, and `<body>` a browser builds, along with the body's default margin.
