import type { LoadBackend } from "./types";
import { loadBackend as loadWasm } from "./wasm-node";

// A dynamic import keeps bundlers from inlining the addon, whose `.node` binary must resolve from
// node_modules at runtime.
const loadNative: LoadBackend = () =>
  import("@takumi-rs/core").catch((cause) => {
    throw new Error(
      "Failed to load the native @takumi-rs/core backend. On a runtime without the native addon, pass a `module` (a WASM binary) to render with the WASM backend instead.",
      { cause },
    );
  });

// WebContainer cannot load native addons, so it takes the WASM backend, detected as Next.js and
// SvelteKit do. `loadWasm` is a static import because Nitro can split a lazy one into a chunk that
// throws `__exportAll is not a function`.
export const loadBackend: LoadBackend = (module) =>
  typeof process !== "undefined" && process.versions?.webcontainer
    ? loadWasm(module)
    : loadNative(module);
