---
"takumi-pdf": minor
---

# Render through characters no font covers with `uncoveredText`

A single character no registered font covers fails the whole render. That is right while you are writing a document, but a server rendering text someone else wrote has no way through.

`uncoveredText: "placeholder"` draws the font's own placeholder glyph, usually an empty box, and `"blank"` draws nothing. Both keep the character's space, so the line does not reflow. The default stays `"error"`.

PDF/A and PDF/UA forbid the glyph `"placeholder"` draws, so pairing the two now fails naming the standard instead of reporting a generic write failure.
