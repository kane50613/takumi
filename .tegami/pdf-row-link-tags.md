---
packages:
  takumi-pdf: patch
---

# An `href` on a table row or row group renders

An `href` on a `tr`, `thead`, `tbody`, or `tfoot` used to fail the whole render with `invalid tag tree: annotation identifier ... doesn't appear in the tag tree`. The link now covers the row or group as it does on other elements, and its `Link` element sits in the last cell the link spans.
