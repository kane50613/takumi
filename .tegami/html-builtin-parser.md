---
packages:
  takumi-html: minor
---

# `from_html` keeps unterminated character references as written

Character references now decode only when they end in `;` and name an HTML4 entity or `&apos;`, as the JS `fromHtml` decodes them, so `&amp` and `&copy` stay as written. `takumi-html` parses with its own parser instead of html5ever, which recovers from less malformed markup: it implies `<html>`, `<head>`, `<body>`, `<tbody>`, and the end tags of `p`, `li`, `dt`, `dd`, `option`, and table rows and cells.
