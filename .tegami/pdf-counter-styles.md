---
packages:
  takumi-pdf:
    type: minor
---

### Count pages in more scripts

Page counters knew seven `@counter-style` names. They now know the digits of eighteen more scripts, from Devanagari and Thai to Tamil and Tibetan, and count through five alphabets including Latin letters, Greek, hiragana and katakana.

A face registered through `fonts` is kept only when its range covers something the page asks for, and a counter's characters appear nowhere in the document. A counter in a style other than decimal now keeps every registered face, so the one it needs survives.
