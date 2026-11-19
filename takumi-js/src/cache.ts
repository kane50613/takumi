import type { CacheStats } from "@takumi-rs/core";
import { defaultRenderer } from "./backend/renderer";

/**
 * What the renderer behind `render` and the other functions keeps, under the keys of the
 * `cache` option a renderer takes. Returns `undefined` until the first render creates that
 * renderer.
 */
export function cacheStats(): CacheStats | undefined {
  return defaultRenderer.cacheStats();
}
