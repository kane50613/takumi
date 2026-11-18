---
packages:
  "takumi-js": patch
  "takumi-pdf": patch
  "takumi-paint": patch
  "@takumi-rs/helpers": patch
---

# `takumi-js`, `takumi-pdf`, and `takumi-paint` tell node trees from JSX the same way

An object counts as a node tree only when its `type` is `container`, `text`, or `image` and it has no `$$typeof`. Two inputs render differently:

- `takumi-js` renders a JSX element whose tag is `container`, `text`, or `image` as JSX. It used to read the element as a node tree.
- `takumi-pdf` and `takumi-paint` render an element object without `$$typeof`, such as `{ type: "p", props: { children: "a" } }`, as JSX. They used to send it to the renderer as a node tree.
