---
packages:
  takumi-core: minor
---

# Support `hyphens` and `hyphenate-character`

`hyphens: none` stops lines from breaking at soft hyphens (U+00AD). `hyphenateCharacter` sets the string a line ends with where it breaks at one. `hyphens: auto` breaks only at soft hyphens, as `manual` does, because takumi has no hyphenation dictionaries.
