---
packages:
  takumi-pdf:
    type: minor
---

### Pick the WASM entry from the bundler's export condition

Bundling `takumi-pdf` broke initialization, because every environment resolved to the Node entry and that entry locates the binary from `import.meta.url`. Vite, Next, workerd and Bun now each get an entry that finds the binary where that bundler puts it.
