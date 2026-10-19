---
packages:
  takumi-html: patch
---

# Build the parsed HTML into an arena

`takumi-html` builds the DOM it parses into one `Vec` instead of `markup5ever_rcdom`'s reference-counted nodes, so it no longer depends on `markup5ever_rcdom` or `xml5ever`.
