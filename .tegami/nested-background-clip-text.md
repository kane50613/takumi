---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Show every `background-clip: text` span's background through nested text

When `background-clip: text` spans nest, each span now shows its background through all the text inside it, outer span first, as in Chrome. Before, only the innermost span's background showed. An inner span's own background now covers the outer span's clipped background, and `text-shadow` paints over the clipped background.
