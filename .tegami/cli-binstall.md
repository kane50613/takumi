---
packages:
  takumi-cli: patch
---

# `cargo binstall takumi-cli` installs a prebuilt binary

Each takumi-cli release on GitHub now carries a binary for macOS, Windows, and Linux, each on x64 and arm64, so `cargo binstall` installs takumi in seconds instead of building it. The Linux binary is statically linked and runs on both glibc and musl.

```sh
cargo binstall takumi-cli
```
