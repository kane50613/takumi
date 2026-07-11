import { loadBackend } from "#backend";
import type { BackendModule } from "./backend/types";

type Imports = Awaited<ReturnType<typeof loadBackend>>;

let importPromise: Promise<Imports> | null = null;

/**
 * Resolves the rendering backend once and caches it. The `#backend` import
 * conditions pick it (napi on Node/Bun, WASM elsewhere), so only the WASM
 * backend accepts a `module`. Keeping both backends behind `#backend` is what
 * keeps each one out of the other's bundle. A failed load clears the cache.
 */
export function getImports(module?: BackendModule): Promise<Imports> {
  importPromise ??= loadBackend(module).catch((error) => {
    importPromise = null;

    throw error;
  });

  return importPromise;
}
