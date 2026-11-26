---
packages:
  "@takumi-rs/helpers": patch
  "@takumi-rs/core": patch
  takumi-pdf: patch
---

# Node trees carrying large images use about the memory of HTML

Rendering a node tree that carries large `data:` URIs, such as one from `fromHtml` or JSX, no longer peaks several times above rendering the same HTML string.
