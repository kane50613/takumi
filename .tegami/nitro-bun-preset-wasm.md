---
packages:
  "@takumi-rs/wasm": patch
  takumi-pdf: patch
  takumi-paint: patch
---

# Render on Nitro's `bun` preset instead of failing

Nitro's `bun` preset used to answer every request with a 500 and `ENOENT ... _libs/@takumi-rs/[object WebAssembly.Module]`. The WASM packages now load their binary through unwasm there, as they do on other Nitro presets.
