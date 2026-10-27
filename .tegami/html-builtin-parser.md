---
packages:
  takumi-html: minor
---

# `from_html` parses markup without html5ever

`takumi-html` now parses with its own parser, which follows the HTML tokenizer but only part of the tree builder's error recovery. It still implies `<html>`, `<head>`, `<body>`, `<tbody>`, and the end tags of `p`, `li`, `dt`, `dd`, `option`, and table rows and cells. Character references now decode only when they end in `;` and name one of HTML4's entities or `&apos;`, as the JS `fromHtml` decodes them, so `&amp` without a semicolon stays as written.
