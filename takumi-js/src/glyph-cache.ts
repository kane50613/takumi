/** The part of a backend this module touches, structural so no entry point's types import `#backend`. */
type GlyphCacheBackend = {
  setGlyphCacheMaxBytes: (bytes: number) => void;
};

let maxBytes: number | undefined;
let loaded: GlyphCacheBackend | undefined;

/**
 * Sets the byte budget of the process-wide glyph caches, 8 MiB by default and `0` to stop caching.
 * Call it before the first render. Raise it for CJK text, whose glyphs outgrow the default.
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
