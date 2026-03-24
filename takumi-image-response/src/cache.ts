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

const stringDataCache = new Map<string, Promise<BinaryData>>();
const functionDataCache = new WeakMap<
  () => BinaryData | Promise<BinaryData>,
  Promise<BinaryData>
>();
const promiseDataCache = new WeakMap<Promise<BinaryData>, Promise<BinaryData>>();

function getOrCreateStringDataCache(
  key: string,
  load: () => BinaryData | Promise<BinaryData>,
): Promise<BinaryData> {
  const cached = stringDataCache.get(key);

  if (cached) {
    return cached;
  }

  const pending = Promise.resolve(load()).catch((error) => {
    stringDataCache.delete(key);
    throw error;
  });

  stringDataCache.set(key, pending);

  return pending;
}

function getOrCreateFunctionDataCache(
  loader: () => BinaryData | Promise<BinaryData>,
): Promise<BinaryData> {
  const cached = functionDataCache.get(loader);

  if (cached) {
    return cached;
  }

  const pending = Promise.resolve(loader()).catch((error) => {
    functionDataCache.delete(loader);
    throw error;
  });

  functionDataCache.set(loader, pending);

  return pending;
}

function getOrCreatePromiseDataCache(promise: Promise<BinaryData>): Promise<BinaryData> {
  const cached = promiseDataCache.get(promise);

  if (cached) {
    return cached;
  }

  const pending = promise.catch((error) => {
    promiseDataCache.delete(promise);
    throw error;
  });

  promiseDataCache.set(promise, pending);

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

  const parts = [font.name, font.weight, font.style];

  if (parts.some((part) => part !== undefined)) {
    return `font:${parts.map((part) => part ?? "").join("-")}`;
  }

  return font;
}

export function createImageCacheKey(image: ImageResponsePersistentImage) {
  return image.key ? `image:${image.key}` : `image:${image.src}`;
}

export async function resolveData(data: BinaryDataLoader, cacheKey?: string) {
  if (typeof data === "function") {
    if (cacheKey) {
      return getOrCreateStringDataCache(cacheKey, data);
    }

    return getOrCreateFunctionDataCache(data);
  }

  if (data instanceof Promise) {
    if (cacheKey) {
      return getOrCreateStringDataCache(cacheKey, () => data);
    }

    return getOrCreatePromiseDataCache(data);
  }

  return data;
}

export async function resolveFont(font: ImageResponseFont): Promise<napi.Font | wasm.Font> {
  if (hasBinaryData(font)) {
    return font;
  }

  const cacheKey = createFontCacheKey(font);

  return {
    ...font,
    data: await resolveData(font.data, typeof cacheKey === "string" ? cacheKey : undefined),
  };
}

export async function resolvePersistentImage(
  image: ImageResponsePersistentImage,
): Promise<napi.ImageSource | wasm.ImageSource> {
  const cacheKey = createImageCacheKey(image);

  return {
    ...image,
    data: await resolveData(image.data, cacheKey),
  };
}
