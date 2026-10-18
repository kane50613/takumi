---
packages:
  takumi-core: patch
---

# Wrap loose table parts in anonymous tables

A `display: table-row` or `table-cell` outside a table, with text or inline elements in it, made rendering panic. Loose table parts now sit in an anonymous table, cells outside a row in an anonymous row, and anything else in a row in an anonymous cell, as Chrome builds them.
