---
packages:
  "@takumi-rs/core": patch
---

# A missing native binding names the package to install

When `@takumi-rs/core` cannot load its native binding, the error names the platform package it looked for and links the troubleshooting guide. Before, it blamed an npm bug for every cause.

```text
// Before
Cannot find native binding. npm has a bug related to optional dependencies (https://github.com/npm/cli/issues/4828). Please try `npm i` again after removing both package-lock.json and node_modules directory.
// After
Cannot find @takumi-rs/core-linux-x64-gnu. Keep @takumi-rs/core out of the bundle, and install @takumi-rs/core-linux-x64-gnu where the app runs. See https://takumi.kane.tw/docs/troubleshooting#cannot-find-native-binding
```

On a platform with no published binding, the error says so and points to `@takumi-rs/wasm`.
