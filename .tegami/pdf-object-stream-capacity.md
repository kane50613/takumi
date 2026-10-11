---
packages:
  "takumi-pdf": patch
---

# Compress the structure tree of documents with more than 65,535 tagged elements

A tagged PDF with more than 65,535 structure elements, such as a table of about 8,000 rows, used to write its structure tree uncompressed. It is now compressed like a smaller document's, which makes an 11,000-row table 3.5 times smaller.
