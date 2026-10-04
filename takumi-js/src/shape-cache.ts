/**
 * Just the part of a backend this module touches. Structural, so the module never
 * imports the backend, which would drag `#backend` into the types of every entry point
 * that re-exports {@link setShapeCacheMaxBytes}.
 */
type ShapeCacheBackend = {
  setShapeCacheMaxBytes: (bytes: number) => void;
};

let maxBytes: number | undefined;
let loaded: ShapeCacheBackend | undefined;

/**
 * Sets the byte budget for the shaped-text cache shared by every render the process
 * makes; `0` disables it. Call before the first render. Defaults to 4 MiB.
 */
export function setShapeCacheMaxBytes(bytes: number): void {
  maxBytes = bytes;
  loaded?.setShapeCacheMaxBytes(bytes);
}

/** Hands the recorded budget to a backend as it finishes loading. */
export function applyShapeCacheMaxBytes(backend: ShapeCacheBackend): void {
  loaded = backend;

  if (maxBytes !== undefined) {
    backend.setShapeCacheMaxBytes(maxBytes);
  }
}
