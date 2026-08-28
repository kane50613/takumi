---
packages:
  "@takumi-rs/core":
    type: patch
---

### Wrap `box-sizing: content-box` text inside its padding

A box with `box-sizing: content-box` and horizontal padding wrapped its text
against the border box, so the text took fewer lines than it needed and
overflowed the bottom of the box.
