let maxBytes: number | undefined;

/**
 * Sets the byte budget shared by the resolved-glyph and glyph-mask caches; `0` stops
 * caching. Defaults to 8 MiB.
 *
 * The caches belong to the backend rather than to a renderer, so this budget covers
 * every render the process makes. It reaches the backend as that loads, and the
 * backend reads it when a cache is first used, so call this before the first render;
 * a later call does not resize a cache already in use.
 *
 * Raise it for scripts with large glyph sets: a CJK outline runs a few kilobytes, so
 * the default holds around a thousand of them and a page of Chinese re-rasterizes
 * glyphs it evicted a moment earlier.
 */
export function setGlyphCacheMaxBytes(bytes: number): void {
  maxBytes = bytes;
}

/**
 * Hands the recorded budget to a freshly loaded backend. Structurally typed so this
 * module never imports the backend, which would drag `#backend` into the types of
 * every entry point that re-exports {@link setGlyphCacheMaxBytes}.
 */
export function applyGlyphCacheMaxBytes(backend: {
  setGlyphCacheMaxBytes: (bytes: number) => void;
}): void {
  if (maxBytes !== undefined) {
    backend.setGlyphCacheMaxBytes(maxBytes);
  }
}
