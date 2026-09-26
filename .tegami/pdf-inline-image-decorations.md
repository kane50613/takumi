---
"takumi-pdf": patch
---

# Render decorated inline images in tagged PDFs

A tagged PDF panicked with "can't start marked content twice" when an inline `<img>` had a background, border, or box shadow. The decorations now paint as artifacts outside the image's own tag.
