---
packages:
  takumi-core: minor
---

# Style list rows with `first:`, `odd:`, and other structural variants in `tw`

The `tw` prop now applies structural variants by the element's place in the tree, or by its contents for `empty:`: `first:`, `last:`, `only:`, `odd:`, `even:`, the `*-of-type:` forms, `empty:`, `nth-3:` and its relatives, `nth-[2n+1]:`, `not-*:`, and arbitrary variants on the element such as `[&:nth-child(3)]:`. They combine with breakpoints, as in `md:first:mt-0`. Before, a class with one of these variants was ignored.

`:empty` in a stylesheet now also matches an element whose only children are text nodes with no characters, as in Chrome.
