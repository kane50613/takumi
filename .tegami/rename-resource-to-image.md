---
packages:
  cargo:takumi-core:
    replay:
      - exit-prerelease(cargo:takumi-core)
---

### Rename `resource` naming to `image`

`Node::resource_urls`/`Style::resource_urls`/`StyleDeclarationBlock::resource_urls` are now
`image_urls`, and `ImageResourceError` is now `ImageError` — everything they cover is image
loading, so the names say so.
