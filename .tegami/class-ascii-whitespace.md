---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# A class list splits on ASCII whitespace only

Selectors used to split an element's `class` attribute on any Unicode whitespace, so `class="a b"` matched `.a` as an ancestor. It now splits on ASCII whitespace, as HTML does, and `a b` is one class. The same applies to the page counter classes in PDFs.
