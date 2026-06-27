---
packages:
  cargo:takumi-core:
    replay:
      - 'exit prerelease: cargo:takumi-core'
---

### Drop `serde_bytes::ByteBuf` from `ImageSourceInput::Buffer`

The `Buffer` variant exposed `serde_bytes::ByteBuf` in the public API. It now
holds a `Vec<u8>` with `#[serde(with = "serde_bytes")]`, keeping the FFI
bytes wire format while keeping `serde_bytes` out of the surface.
