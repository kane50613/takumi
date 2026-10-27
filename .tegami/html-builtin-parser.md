---
packages:
  takumi-html: minor
---

# `takumi-html` parses with its own parser instead of html5ever

It recovers from less malformed markup than html5ever. It implies `<html>`, `<head>`, `<body>`, `<tbody>`, and the end tags of `p`, `li`, `dt`, `dd`, `option`, and table rows and cells, but does not reconstruct misnested formatting elements or move stray table content out of the table.
