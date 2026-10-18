---
packages:
  takumi-core: patch
---

# Transform text case as browsers do

`text-transform: capitalize` no longer lowercases the rest of each word, and a word split across inline elements is capitalized once. `uppercase` and `lowercase` follow the `lang` of Turkish, Azerbaijani, Greek, Lithuanian, and Dutch text, and uppercased Georgian keeps its usual letters.
