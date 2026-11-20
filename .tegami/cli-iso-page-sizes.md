---
packages:
  takumi-cli: minor
---

# `takumi pdf --page` takes every ISO A and B size

CSS names only A3 to A5, B4, and B5, so `--page A1` failed and a poster needed its size in millimetres. `--page` now also takes A0 to A10 and B0 to B10, with `landscape` or `portrait`.

```diff
- takumi pdf poster.html -o poster.pdf --page "594mm 841mm"
+ takumi pdf poster.html -o poster.pdf --page A1
```
