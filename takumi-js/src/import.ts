import { loadBackend } from "#backend";
import type { BackendModule } from "./backend/types";

export type Imports = Awaited<ReturnType<typeof loadBackend>>;

let backendPromise: Promise<Imports> | null = null;
let wasmPromise: Promise<Imports> | null = null;

/**
 * Resolves the rendering backend and caches it, one slot per strategy. With no
 * `module`, the `#backend` import conditions pick it (napi on Node/Bun, WASM
 * elsewhere). An explicit `module` is a WASM binary, so it forces WASM — the
 * escape hatch for a target that can't load the native addon. The slots are
 * separate so a forced-WASM call never gets a previously cached napi backend
 * (and vice versa). A failed load clears its slot.
 */
export function getImports(module?: BackendModule): Promise<Imports> {
  if (module === undefined) {
    backendPromise ??= loadBackend().catch((error) => {
      backendPromise = null;

      throw error;
    });

    return backendPromise;
  }

  wasmPromise ??= import("./backend/wasm-init")
    .then(({ initWasm }) => initWasm(module))
    .catch((error) => {
      wasmPromise = null;

      throw error;
    });

  return wasmPromise;
}
