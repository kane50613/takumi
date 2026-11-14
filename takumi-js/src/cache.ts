import type { CacheOptions } from "@takumi-rs/core";

let cacheOptions: CacheOptions | undefined;

/**
 * Sets the memory the renderer behind `render` and the other functions keeps across renders. Call it
 * before the first render, since the renderer keeps the options it was created with.
 */
export function setCacheOptions(options: CacheOptions): void {
  cacheOptions = options;
}

/** @deprecated Use `setCacheOptions({ budgets: { glyphs } })`. */
export function setGlyphCacheMaxBytes(bytes: number): void {
  cacheOptions = { ...cacheOptions, budgets: { ...cacheOptions?.budgets, glyphs: bytes } };
}

/** The cache options the managed renderer is created with. */
export function managedCacheOptions(): CacheOptions | undefined {
  return cacheOptions;
}
