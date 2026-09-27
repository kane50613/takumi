---
packages:
  "takumi-pdf": patch
---

# Fade blurred text shadows in PDF output

A `text-shadow` with a blur radius now fades out in PDF output through the same stepped bands a blurred `box-shadow` uses. Before, it drew as a sharp copy of the text.
