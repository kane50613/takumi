---
"takumi": patch
---

# Stop anonymous boxes inheriting their parent's box model

Bare text inside a padded `display: flex` container measured against a content box narrowed by the parent's padding again, so a line that fit was laid out as two.
