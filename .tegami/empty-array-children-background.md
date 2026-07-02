---
packages:
  npm:@takumi-rs/helpers:
    replay:
      - "exit prerelease: npm:@takumi-rs/helpers"
---

### Keep elements as containers when children carry no text

An element whose children resolved to a textless iterable (e.g. `{[]}`) became an
empty text node instead of a container, so its `background` and other box styles
never painted. Such elements now stay containers.
