---
packages:
  cargo:takumi:
    replay:
      - exit-prerelease(cargo:takumi)
---

### Make `:lang()` actually match

`:lang()` parsed but never matched, like every other pseudo-class the engine treats as
stateful. It needs no live state, only the `lang` attribute inherited up the tree, which a
static render already has. It now matches BCP-47 basic filtering (`:lang(zh-Hant)`, comma-separated
ranges, `*`) against the nearest ancestor-or-self with a `lang` set — the standards-based way to
route different fonts to different languages on the same page, e.g. `:lang(ja) { font-family:
"Noto Sans JP" }` alongside `:lang(zh-Hant) { font-family: "Noto Sans TC" }`.
