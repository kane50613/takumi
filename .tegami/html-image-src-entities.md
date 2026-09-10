---
"takumi": patch
---

# Decode character references in `fromHtml` image sources

An `img` whose `src` holds `&amp;` now yields the decoded URL, matching `attributes.src`.
