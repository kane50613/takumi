import type { CacheOptions } from "@takumi-rs/core";
import { defaultRenderer } from "./backend/renderer";

/**
 * Sets the memory the renderer behind `render` and the other functions keeps across renders. A
 * renderer that already exists takes the new options at once, and a smaller size releases what no
 * longer fits.
 */
export function setCacheOptions(options: CacheOptions): void {
  defaultRenderer.setCacheOptions(options);
}

/** @deprecated Use `setCacheOptions({ budgets: { glyphs } })`. */
export function setGlyphCacheMaxBytes(bytes: number): void {
  const options = defaultRenderer.cacheOptions;

  setCacheOptions({ ...options, budgets: { ...options?.budgets, glyphs: bytes } });
}
