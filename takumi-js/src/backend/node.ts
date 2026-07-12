import type { LoadBackend } from "./types";

// Dynamic import so bundlers don't inline the native addon — its `.node` binary
// must resolve from node_modules at runtime, which a bundled-in copy breaks.
// Selected by the `node`/`bun` condition, so it never reaches a worker/edge bundle.
const loadNative: LoadBackend = () =>
  import("@takumi-rs/core").catch((cause) => {
    throw new Error(
      "Failed to load the native @takumi-rs/core backend. On a runtime without the native addon, pass a `module` (a WASM binary) to render with the WASM backend instead.",
      { cause },
    );
  });

// WebContainer can't load native addons, and unbundled runs (e.g. `nitro dev`
// externalizing this package) resolve `#backend` with Node's default conditions,
// so the `node` condition lands here even when the host set `unwasm`. Detect it
// (the same signal Next.js and SvelteKit use) and reroute to the WASM backend.
export const loadBackend: LoadBackend = (module) =>
  typeof process !== "undefined" && process.versions?.webcontainer
    ? import("./wasm").then((backend) => backend.loadBackend(module))
    : loadNative(module);
