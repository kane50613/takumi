---
packages:
  takumi-core: patch
---

# An `@page` rule's `size` no longer warns as an unsupported property

`size` in an `@page` rule, including the one `takumi pdf --page` writes, set the page size and also raised `ignored the property \`size\``, so `--deny-warnings` failed on a valid document. It now sets the page size without a warning.
