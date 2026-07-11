import type { LoadBackend } from "./types";

const NO_NATIVE_ADDON =
  "Failed to load the native @takumi-rs/core backend. On a runtime without the native addon, build a WASM renderer with `createRenderer` from `takumi-js/wasm` and pass it as `renderer`.";

// Dynamic import so bundlers don't inline the native addon — its `.node` binary
// must resolve from node_modules at runtime, which a bundled-in copy breaks.
// Selected by the `node`/`bun` condition, so it never reaches a worker/edge bundle.
export const loadBackend: LoadBackend = (module) => {
  if (module !== undefined) {
    return Promise.reject(
      new Error(
        "The native backend takes no WASM `module`. Build a renderer with `createRenderer` from `takumi-js/wasm` and pass it as `renderer` instead.",
      ),
    );
  }

  return import("@takumi-rs/core").catch((cause) => {
    throw new Error(NO_NATIVE_ADDON, { cause });
  });
};
