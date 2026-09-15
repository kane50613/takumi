---
packages:
  "takumi-pdf": minor
---

# Render uncovered characters with `uncoveredText`

A single character no registered font covers used to fail the whole render, which leaves a server rendering text someone else wrote with no way through. `uncoveredText: "placeholder"` draws the font's glyph 0 instead, and `"blank"` draws nothing. Both keep the character's width, so the line does not reflow. The default stays `"error"`.

Every PDF/A level and both PDF/UA levels forbid glyph 0, so `"placeholder"` is now rejected there by name instead of failing as a generic write error.
