---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Size inline boxes from their own font, as Chrome does

A span's background and border now take the height of the span's own font on every line, even when it holds only other spans or nothing but padding. A span with a larger font than its text also makes its line as tall as Chrome does.
