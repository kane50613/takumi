---
packages:
  takumi-core: patch
  "@takumi-rs/helpers": patch
---

# Block elements isolate their bidi text

The default styles give `div`, `p`, headings, lists, table parts, `bdi`, `output`, and the other elements HTML's rendering rules list `unicode-bidi: isolate`, as Chrome's do. An element made inline by CSS keeps its text apart from the bidi ordering of the text around it.

```html
<div>שלום <section style="display: inline">abc 123</section> עולם</div>
<!-- Before: the inline section's text is reordered with the Hebrew around it -->
<!-- After: it stays one isolated run -->
```
