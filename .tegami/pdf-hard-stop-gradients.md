---
packages:
  takumi-pdf: patch
---

# Gradients to transparent keep their colour in PDFs

A gradient that fades to `transparent` no longer darkens through black. A gradient whose stops are each opaque or transparent, such as the highlighter `linear-gradient(transparent 55%, yellow 55%)`, draws without a soft mask, which some PDF readers leave out.
