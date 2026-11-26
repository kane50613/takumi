---
packages:
  takumi-core: patch
---

# Font size keywords follow the default font size

`font-size: medium` and the other absolute size keywords take their sizes from the default font size, as browsers do, instead of from the root element's font size.

```css
html { font-size: 20px; }
p { font-size: medium; }
/* Before: the paragraph's text is 20px */
/* After: the paragraph's text is 16px, and `small` is 13px */
```
