import { loadBackend } from "#backend";
import type { BackendModule, LoadBackend } from "./backend/types";

type Imports = Awaited<ReturnType<typeof loadBackend>>;

let importPromise: Promise<Imports> | null = null;

/**
 * Resolves the rendering backend once and caches it. With no `module`, the
 * `#backend` import conditions pick it (napi on Node/Bun, WASM elsewhere). An
 * explicit `module` is a WASM binary, so it forces WASM — the escape hatch for a
 * Node target that can't load the native addon. A failed load clears the cache.
 */
export function getImports(module?: BackendModule): Promise<Imports> {
  importPromise ??= (
    module === undefined
      ? loadBackend()
      : import("./backend/wasm").then((wasm: { loadBackend: LoadBackend }) =>
          wasm.loadBackend(module),
        )
  ).catch((error) => {
    importPromise = null;

    throw error;
  });

  return importPromise;
}
