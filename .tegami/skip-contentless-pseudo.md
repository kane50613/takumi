---
packages:
  takumi-core: patch
---

# Style pages with a CSS reset faster

A rule like `*, ::before, ::after { box-sizing: border-box }`, which Tailwind's Preflight and most resets open with, no longer makes takumi compute a full style for every element's `::before` and `::after`. Those styles are now skipped when no matched rule sets `content`, since such a pseudo-element generates no box. Rendering 400 elements under such a reset is 27% faster, with identical output.
