---
packages:
  "takumi-core": patch
  "takumi": patch
  "takumi-pdf": patch
---

# Stop leaking a direction mark into PDF text

Paragraphs set their direction directly instead of starting with an invisible U+200E or U+200F mark. Text extracted from a PDF no longer contains the mark.
