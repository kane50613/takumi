---
packages:
  takumi-html: patch
---

# Parse a document that starts with a byte order mark as a document

`from_html` now drops a leading byte order mark, as a browser does when it decodes a page. A document saved with one used to parse as a fragment, so it lost its `<body>` and the body's margin.
