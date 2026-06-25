import type { LoadBackend } from "./types";

// Dynamic import so bundlers don't inline the native addon — its `.node` binary
// must resolve from node_modules at runtime, which a bundled-in copy breaks.
// Selected by the `node`/`bun` condition, so it never reaches a worker/edge bundle.
export const loadBackend: LoadBackend = () => import("@takumi-rs/core");
