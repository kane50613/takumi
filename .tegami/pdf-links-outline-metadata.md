---
packages:
  "cargo:takumi-pdf": minor
  "cargo:takumi-core": minor
---

### Add PDF hyperlinks, outline, and document metadata

Anchors with an `href` become clickable link annotations, at the box for block anchors and per text run for inline ones. An `outline: true` option builds PDF bookmarks from `h1`–`h6` headings, and a `metadata` option fills the document's title, description, authors, keywords, and creator.
