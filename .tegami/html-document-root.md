---
packages:
  takumi-html:
    type: minor
---

### Keep the `<html>` root when parsing a document

A source starting with `<html>` is parsed as a document, so the tree keeps that element along with `<body>` and the styles on both. It used to be parsed as a fragment, which dropped the wrappers. Anything else is still a fragment and gains no wrappers of its own.
