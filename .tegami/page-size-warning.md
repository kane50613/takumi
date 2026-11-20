---
packages:
  takumi-core: patch
---

# An `@page` rule's `size` no longer warns as an unsupported property

`size` in an `@page` rule, including the one `takumi pdf --page` writes, set the page size and also raised a warning, so `--deny-warnings` failed on a valid document. It now sets the page size without a warning.

```diff
  $ takumi pdf report.html -o report.pdf --page A4 --deny-warnings
- warning: ignored the property `size` at line 1, column 6 of its style sheet or `style` attribute, since takumi does not support it. Check its spelling.
- error: `--deny-warnings` is set and the command raised 1 warning, so it wrote nothing.
```
