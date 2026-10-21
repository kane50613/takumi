---
packages:
  takumi-core: minor
---

# Style `::before` and `::after` from `tw` with `before:`, `after:`, and `content-[…]`

The `tw` prop now takes `before:` and `after:` variants, which style the element's `::before` and `::after`, and `content-['…']`, `content-[attr(…)]`, and `content-none` set what they show. They combine with structural variants and breakpoints, as in `first:before:content-['→']`.
