import type { LoadBackend } from "./types";

// Dynamic imports keep both backends out of the eager entry: the native addon's
// `.node` binary must resolve from node_modules at runtime (a bundled-in copy
// breaks it), and the WASM escape hatch stays in a chunk that only a caller
// passing `module` loads.
// Selected by the `node`/`bun` condition, so neither reaches a worker/edge bundle.
export const loadBackend: LoadBackend = (module) => {
  if (module !== undefined) {
    return import("./wasm-init").then(({ initWasm }) => initWasm(module));
  }

  return import("@takumi-rs/core").catch((cause) => {
    throw new Error(
      "Failed to load the native @takumi-rs/core backend. On a runtime without the native addon, pass a `module` (a WASM binary) to render with the WASM backend instead.",
      { cause },
    );
  });
};
