import { loadBackend } from "#backend";
import type { BackendModule } from "./backend/types";

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
      : import("./backend/wasm-init").then(({ initWasm }) => initWasm(module))
  ).catch((error) => {
    importPromise = null;

    throw error;
  });

  return importPromise;
}

/**
 * Sets the byte budget shared by the resolved-glyph and glyph-mask caches; `0` stops
 * caching. Defaults to 8 MiB.
 *
 * The caches belong to the backend rather than to a renderer, so this budget covers
 * every render the process makes, and the value is read the first time a cache is
 * used. Await this before the first render.
 *
 * Raise it for scripts with large glyph sets: a CJK outline runs a few kilobytes, so
 * the default holds around a thousand of them and a page of Chinese re-rasterizes
 * glyphs it evicted a moment earlier.
 */
export async function setGlyphCacheMaxBytes(
  bytes: number,
  options?: { module?: BackendModule },
): Promise<void> {
  const backend = await getImports(options?.module);

  backend.setGlyphCacheMaxBytes(bytes);
}
