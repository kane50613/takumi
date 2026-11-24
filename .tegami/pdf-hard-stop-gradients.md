---
packages:
  takumi-pdf: patch
---

# Gradients with transparency draw as CSS does in PDFs

Gradients with transparent or translucent stops now draw in PDFs as they do in browsers:

- A gradient whose alpha changes interpolates premultiplied, as CSS does, so a fade toward `transparent` no longer darkens through black.
- A linear gradient whose stops are each opaque or transparent, with the alpha changing only at hard stops, such as the highlighter `linear-gradient(transparent 55%, yellow 55%)`, draws without a soft mask. Apple's PDF renderer can draw a soft mask wrong.
