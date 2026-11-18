/** The part of a backend this module touches, structural so no entry point's types import `#backend`. */
type GlyphCacheBackend = {
  setGlyphCacheMaxBytes: (bytes: number) => void;
};

let maxBytes: number | undefined;
let loaded: GlyphCacheBackend | undefined;

/**
 * Keeps no glyphs when `bytes` is `0`, and glyphs again otherwise, in the renderers created after
 * this call. Glyphs share the renderer's `cache.maxBytes` with every other kind of data, so a
 * size no longer bounds them apart. Call it before the first render.
 */
export function setGlyphCacheMaxBytes(bytes: number): void {
  maxBytes = bytes;
  loaded?.setGlyphCacheMaxBytes(bytes);
}

/** Hands the recorded budget to a backend as it finishes loading. */
export function applyGlyphCacheMaxBytes(backend: GlyphCacheBackend): void {
  loaded = backend;

  if (maxBytes !== undefined) {
    backend.setGlyphCacheMaxBytes(maxBytes);
  }
}
