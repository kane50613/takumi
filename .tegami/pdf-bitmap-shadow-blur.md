---
packages:
  "takumi-pdf": patch
---

# Blur the shadow of bitmap emoji in PDF output

A blurred `text-shadow` on a bitmap glyph, such as Noto Color Emoji, now draws its silhouette blurred as Chrome blurs it. Before, PDF output drew no shadow for it.
