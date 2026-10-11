---
packages:
  "takumi": patch
---

# Pick PNG settings with the encoder that writes the file

The sample a PNG encode tries its settings on is now deflated the way the final file is, so the choice between them is more accurate and quicker to make. Across real templates, encoding takes 4% less time and files come out 0.2% smaller.
