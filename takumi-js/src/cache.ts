import type { CacheOptions } from "@takumi-rs/core";

let cacheOptions: CacheOptions | undefined;

/**
 * Sets the byte budgets of what the renderer behind `render` and the other functions keeps across
 * renders. Call it before the first render, since the renderer keeps the budgets it was created with.
 */
export function setCacheOptions(options: CacheOptions): void {
  cacheOptions = options;
}

/** @deprecated Use `setCacheOptions({ glyphs })`. */
export function setGlyphCacheMaxBytes(bytes: number): void {
  cacheOptions = { ...cacheOptions, glyphs: bytes };
}

/** The budgets the managed renderer is created with. */
export function managedCacheOptions(): CacheOptions | undefined {
  return cacheOptions;
}
