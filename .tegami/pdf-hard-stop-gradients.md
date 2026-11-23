---
packages:
  takumi-pdf: patch
---

# Gradients with transparency draw as CSS does in PDFs

Gradients whose alpha changes interpolate premultiplied, as CSS does, so a fade toward `transparent` no longer darkens through black. A gradient whose stops are each opaque or transparent, such as the highlighter `linear-gradient(transparent 55%, yellow 55%)`, draws without a soft mask, which Apple's PDF renderer can draw wrong.
