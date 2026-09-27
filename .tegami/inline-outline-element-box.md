---
packages:
  "takumi": patch
---

# Outline an inline element around its whole box

An inline element's `outline` now wraps its border box on each line, including its padding, border and nested elements, and a one-line outline follows its `border-radius`. Before, an element whose text was split by a nested element, such as `plain <b>bold</b> text`, painted no outline at all.
