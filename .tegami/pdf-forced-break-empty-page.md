---
"takumi-pdf": patch
---

# Forced page breaks no longer open empty pages

A `break-before: page` or `break-after: page` with only spacing beside it on its page is dropped, and trailing spacing past the last content box no longer opens a page.
