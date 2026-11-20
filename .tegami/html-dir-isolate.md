---
packages:
  takumi-core: patch
---

# `dir`, `<bdi>`, and `<bdo>` isolate their text as in a browser

An element with a `dir` attribute and a `<bdi>` now isolate their text from the text around them, and a `<bdo>` overrides the order of its text. `dir="auto"` and `<bdi>` also take their direction from their content. This follows the HTML default styles, so a date wrapped in one of them keeps its order next to Arabic text.

```html
<p dir="rtl">التاريخ: <bdi>2026-11-19</bdi></p>
```

- Before, the date drew as `19-11-2026`, since the `<bdi>` did not isolate it from the Arabic before it.
- Now it draws as `2026-11-19`, as in a browser.
- A date with no isolating element still reorders after Arabic text, as it does in a browser.
