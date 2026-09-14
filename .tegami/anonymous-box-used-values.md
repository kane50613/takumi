---
"takumi": patch
---

# Take an anonymous box's border width down to its used value

An anonymous box skipped the used-value pass, so it reported the initial `medium` border width even though it has no border style to render it with.
