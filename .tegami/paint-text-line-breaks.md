---
packages:
  takumi-cli: patch
---

# `takumi paint` keeps the break between wrapped lines

A text that wrapped onto several lines used to print with its lines run together, so `Takumi CLI, tested by an agent` wrapped after `tested` printed as `"Takumi CLI, testedby an agent"`. Each line break now prints as `\n`.

```diff
- text p 8,8 284x82 "Takumi CLI, testedby an agent" Geist 32px #000000
+ text p 8,8 284x82 "Takumi CLI, tested\nby an agent" Geist 32px #000000
```
