---
packages:
  "takumi": patch
---

# Blur large surfaces across threads

`filter: blur()`, `backdrop-filter: blur()` and large shadows blur on several threads in native builds, so a page with big blurred backgrounds renders 14-32% faster. Small blurs, such as a text shadow per glyph, stay on one thread.
