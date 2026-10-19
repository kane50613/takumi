---
packages:
  takumi-core: patch
---

# Drop line and column from CSS value errors

`cssparser` 0.38 no longer tracks where a parse error happened, so an invalid style value reports the token it stopped at but no longer its line and column.
