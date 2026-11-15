---
packages:
  takumi-js: minor
---

# The `takumi-wasm` export condition renders on WebAssembly in Node.js

Under the `takumi-wasm` condition, `takumi-js` picks the WebAssembly backend in Node.js and Bun as well, so a server can ship without the native addon. Pass it as `node --conditions=takumi-wasm`, or set it in your bundler. See [Use WebAssembly in Node.js](https://takumi.kane.tw/docs#use-webassembly-in-nodejs) for Vite and esbuild.
