---
packages:
  takumi-core: patch
---

# Draw an inline image inside its margins

An `<img>` or other replaced element in a line of text was drawn across its margins, stretched and with its border and background reaching into them. It now draws in its border box, and its margins stay empty space on the line.
