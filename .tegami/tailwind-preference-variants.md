---
packages:
  takumi-core: patch
---

# Tailwind preference variants and breakpoint order

`motion-safe:`, `motion-reduce:`, `contrast-more:`, `contrast-less:`, `forced-colors:`, and `inverted-colors:` now apply as in a default Chrome, and `not-hover:`, `not-focus:`, and the other `not-` variants of a state no static render is in apply. `min-[900px]:` and `sm:` now sort by unit before width as Tailwind does, and `min-*` and `max-*` no longer both match at a fractional width.
