---
packages:
  takumi-core: patch
---

# Tailwind `max-*` and `min-*` breakpoints, and ties that sort by number

The `tw` prop reads these as Tailwind does:

- `max-md:`, `min-[600px]:`, and `max-[800px]:` compare the viewport width, and `sm:max-md:` applies only while both hold.
- When two utilities set the same property, digit runs in their names compare as numbers, so `p-2 p-10` applies `p-10`.
