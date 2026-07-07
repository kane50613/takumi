---
packages:
  npm:@takumi-rs/image-response:
    replay:
      - exit-prerelease(npm:@takumi-rs/image-response)
---

### Drop the `./wasm` export

`@takumi-rs/image-response/wasm` aliased the same file as the root entry. Import from
`@takumi-rs/image-response`.
