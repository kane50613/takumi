---
packages:
  "cargo:takumi-core": patch
---

### Key the glyph caches on font content instead of blob identity

`Blob::new` draws its id from a global counter, and that id is part of the key for the shared resolved-glyph and glyph-mask caches, as well as parley's shaping data cache. Registering the same face again produced a fresh id, so a second renderer, or one rebuilt to reclaim memory, missed every glyph the face had already resolved and filled the budget with entries nothing would hit again. The id is now a hash of the decoded font bytes, so identical faces share cache entries no matter how often they are registered.
