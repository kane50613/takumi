import { loadBackend } from "#backend";
import type { BackendModule } from "./backend/types";

type Imports = Awaited<ReturnType<typeof loadBackend>>;

let importPromise: Promise<Imports> | null = null;

/**
 * Resolves the rendering backend once and caches it. Which backend loads is
 * decided by the package's `#backend` import conditions, so the bundler (or
 * runtime) picks napi on Node/Bun and WASM everywhere else — no native binary
 * ever reaches a worker/edge bundle, and no runtime sniffing is needed.
 */
export function getImports(module?: BackendModule): Promise<Imports> {
  importPromise ??= loadBackend(module);

  return importPromise;
}
