---
packages:
  takumi-core: patch
---

# Parse each style sheet on its own

`StyleSheet::parse_list` and its variants parse each sheet separately, as Chrome does, so an unclosed comment or block in one `<style>` element no longer swallows the sheets after it. Their rules still apply in order, and `@layer` order still carries from one sheet to the next.
