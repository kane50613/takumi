import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";

export type BinaryData = Uint8Array | ArrayBuffer;
export type BinaryDataLoader =
  | BinaryData
  | Promise<BinaryData>
  | (() => BinaryData | Promise<BinaryData>);
export type ImageResponseFont =
  | BinaryData
  | {
      data: BinaryDataLoader;
      key?: string;
      name?: string;
      style?: napi.FontDetails["style"] | wasm.FontDetails["style"];
      weight?: number;
    };
export type ImageResponsePersistentImage = {
  data: BinaryDataLoader;
  key?: string;
  src: string;
};

export type ResourceCache = {
  functionDataCache: WeakMap<() => BinaryData | Promise<BinaryData>, Promise<BinaryData>>;
  loadedFontKeys: Set<string>;
  loadedFontObjects: WeakSet<object>;
  loadedImageKeys: Set<string>;
  loadedImageObjects: WeakSet<object>;
  promiseDataCache: WeakMap<Promise<BinaryData>, Promise<BinaryData>>;
  renderer?: napi.Renderer | wasm.Renderer;
  stringDataCache: Map<string, Promise<BinaryData>>;
};

export function createResourceCache(): ResourceCache {
  return {
    functionDataCache: new WeakMap(),
    loadedFontKeys: new Set(),
    loadedFontObjects: new WeakSet(),
    loadedImageKeys: new Set(),
    loadedImageObjects: new WeakSet(),
    promiseDataCache: new WeakMap(),
    stringDataCache: new Map(),
  };
}

function getOrCreateStringDataCache(
  cache: ResourceCache,
  key: string,
  load: () => BinaryData | Promise<BinaryData>,
): Promise<BinaryData> {
  const cached = cache.stringDataCache.get(key);

  if (cached) {
    return cached;
  }

  const pending = Promise.resolve(load()).catch((error) => {
    cache.stringDataCache.delete(key);
    throw error;
  });

  cache.stringDataCache.set(key, pending);

  return pending;
}

function getOrCreateFunctionDataCache(
  cache: ResourceCache,
  loader: () => BinaryData | Promise<BinaryData>,
): Promise<BinaryData> {
  const cached = cache.functionDataCache.get(loader);

  if (cached) {
    return cached;
  }

  const pending = Promise.resolve(loader()).catch((error) => {
    cache.functionDataCache.delete(loader);
    throw error;
  });

  cache.functionDataCache.set(loader, pending);

  return pending;
}

function getOrCreatePromiseDataCache(
  cache: ResourceCache,
  promise: Promise<BinaryData>,
): Promise<BinaryData> {
  const cached = cache.promiseDataCache.get(promise);

  if (cached) {
    return cached;
  }

  const pending = promise.catch((error) => {
    cache.promiseDataCache.delete(promise);
    throw error;
  });

  cache.promiseDataCache.set(promise, pending);

  return pending;
}

function hasBinaryData(value: ImageResponseFont): value is BinaryData {
  return value instanceof Uint8Array || value instanceof ArrayBuffer;
}

export function createFontCacheKey(font: ImageResponseFont) {
  if (hasBinaryData(font)) {
    return font;
  }

  if (font.key) {
    return `font:${font.key}`;
  }

  return font;
}

export function createImageCacheKey(image: ImageResponsePersistentImage) {
  return image.key ? `image:${image.key}` : image;
}

export async function resolveData(cache: ResourceCache, data: BinaryDataLoader, cacheKey?: string) {
  if (typeof data === "function") {
    if (cacheKey) {
      return getOrCreateStringDataCache(cache, cacheKey, data);
    }

    return getOrCreateFunctionDataCache(cache, data);
  }

  if (data instanceof Promise) {
    if (cacheKey) {
      return getOrCreateStringDataCache(cache, cacheKey, () => data);
    }

    return getOrCreatePromiseDataCache(cache, data);
  }

  return data;
}

export async function resolveFont(
  cache: ResourceCache,
  font: ImageResponseFont,
): Promise<napi.Font | wasm.Font> {
  if (hasBinaryData(font)) {
    return font;
  }

  const cacheKey = createFontCacheKey(font);

  return {
    ...font,
    data: await resolveData(cache, font.data, typeof cacheKey === "string" ? cacheKey : undefined),
  };
}

export async function resolvePersistentImage(
  cache: ResourceCache,
  image: ImageResponsePersistentImage,
): Promise<napi.ImageSource | wasm.ImageSource> {
  const cacheKey = createImageCacheKey(image);

  return {
    ...image,
    data: await resolveData(cache, image.data, typeof cacheKey === "string" ? cacheKey : undefined),
  };
}
