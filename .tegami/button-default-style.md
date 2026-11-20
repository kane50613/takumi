---
packages:
  takumi-core: patch
---

# `<button>` draws as a button

A `<button>` had no default style, so it laid out as inline text: `width` did nothing, a background drew only behind its text, and its label sat at the start. It now takes Chromium's default button style, an inline block with a centered label, `padding: 1px 6px`, a 2px outset border, and a light gray background, which an author's styles override.

```html
<button style="width: 200px; background: #4338ca; color: #fff">Sign up</button>
```

The button above is now 200px wide, with its label centered.
