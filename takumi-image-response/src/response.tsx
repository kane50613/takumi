import { fetchResources } from "@takumi-rs/helpers";
import { type EmojiType, extractEmojis } from "@takumi-rs/helpers/emoji";
import { type FromJsxOptions, fromJsx } from "@takumi-rs/helpers/jsx";
import type { ReactNode } from "react";
import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";
import {
  createFontCacheKey,
  createImageCacheKey,
  createResourceCache,
  resolveFont,
  resolvePersistentImage,
  type ImageResponseFont,
  type ImageResponsePersistentImage,
  type ResourceCache,
} from "./cache";
import { getImports, type Imports } from "./import";

const defaultFormat = "webp";

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}

type RenderOptions = napi.RenderOptions | wasm.RenderOptions;
type ManagedRendererOptions = {
  fonts?: ImageResponseFont[];
  /**
   * Only supported by the native `@takumi-rs/core` renderer.
   * This option is ignored when using the WASM renderer.
   */
  loadDefaultFonts?: boolean;
  persistentImages?: ImageResponsePersistentImage[];
  /**
   * @description The WebAssembly module to use for the renderer. If not provided, the default resolving strategy will be used.
   */
  module?: wasm.InitInput | Promise<wasm.InitInput> | { default: wasm.InitInput };
};

type ImageResponseOptionsWithRenderer = ResponseInit &
  RenderOptions & {
    renderer: napi.Renderer | wasm.Renderer;
    signal?: AbortSignal;
    jsx?: FromJsxOptions;
    emoji?: EmojiType | "from-font";
    onError?: (error: unknown) => void | Promise<void>;
  };

export type ImageResponseOptionsWithoutRenderer = Omit<
  ImageResponseOptionsWithRenderer,
  "renderer"
> &
  ManagedRendererOptions;

export type ImageResponseOptions =
  | ImageResponseOptionsWithRenderer
  | ImageResponseOptionsWithoutRenderer;

export type ImageResponseResult = Response & {
  readonly ready: Promise<void>;
};

export type ImageResponseFactory = (
  component: ReactNode,
  options?: ImageResponseOptions,
) => ImageResponseResult;

function hasLoadedResource(
  key: object | string,
  loadedKeys: Set<string>,
  loadedObjects: WeakSet<object>,
) {
  if (typeof key === "string") {
    return loadedKeys.has(key);
  }

  return loadedObjects.has(key);
}

function markLoadedResource(
  key: object | string,
  loadedKeys: Set<string>,
  loadedObjects: WeakSet<object>,
) {
  if (typeof key === "string") {
    loadedKeys.add(key);
    return;
  }

  loadedObjects.add(key);
}

function mergeOptions(
  defaultOptions: ImageResponseOptionsWithoutRenderer | undefined,
  options: ImageResponseOptions | undefined,
): ImageResponseOptions | undefined {
  if (!defaultOptions) {
    return options;
  }

  if (!options) {
    return defaultOptions;
  }

  const headers = new Headers(defaultOptions.headers);

  if (options.headers) {
    const optionHeaders = new Headers(options.headers);

    optionHeaders.forEach((value, key) => {
      headers.set(key, value);
    });
  }

  if ("renderer" in options) {
    return {
      ...defaultOptions,
      ...options,
      headers,
    };
  }

  return {
    ...defaultOptions,
    ...options,
    fonts: options.fonts ?? defaultOptions.fonts,
    headers,
    persistentImages: options.persistentImages ?? defaultOptions.persistentImages,
    stylesheets: [...(defaultOptions.stylesheets ?? []), ...(options.stylesheets ?? [])],
  };
}

const contentTypeMap: Record<NonNullable<RenderOptions["format"]>, string> = {
  png: "image/png",
  jpeg: "image/jpeg",
  webp: "image/webp",
  raw: "application/octet-stream",
};

async function renderComponent(
  component: ReactNode,
  options: ImageResponseOptions | undefined,
  imports: Imports,
  renderer: napi.Renderer | wasm.Renderer,
) {
  const { node: originalNode, stylesheets } = await fromJsx(component, options?.jsx);
  const node =
    options?.emoji && options.emoji !== "from-font"
      ? extractEmojis(originalNode, options.emoji)
      : originalNode;
  const fetchedResources =
    options?.fetchedResources !== undefined
      ? options.fetchedResources
      : await fetchResources(imports.extractResourceUrls(node));

  const renderOptions = {
    ...options,
    fetchedResources,
    format: options?.format ?? defaultFormat,
    stylesheets: [...(options?.stylesheets ?? []), ...stylesheets],
  };

  return renderer.render(node, renderOptions, options?.signal);
}

function defaultErrorHandler(error: unknown) {
  console.error("Takumi failed to render image.");
  console.error(error);
}

async function loadRendererResources(
  renderer: napi.Renderer | wasm.Renderer,
  options: ImageResponseOptionsWithoutRenderer | undefined,
  cache: ResourceCache,
) {
  const tasks: Promise<unknown>[] = [];

  if (options?.fonts && options.fonts.length > 0) {
    const resolvedFonts = await Promise.all(
      options.fonts.map(async (font) => ({
        cacheKey: createFontCacheKey(font),
        font: await resolveFont(cache, font),
      })),
    );

    if ("loadFonts" in renderer) {
      const filteredFonts = resolvedFonts.filter(({ cacheKey }) => {
        if (hasLoadedResource(cacheKey, cache.loadedFontKeys, cache.loadedFontObjects)) {
          return false;
        }

        markLoadedResource(cacheKey, cache.loadedFontKeys, cache.loadedFontObjects);
        return true;
      });

      if (filteredFonts.length > 0) {
        tasks.push(renderer.loadFonts(filteredFonts.map(({ font }) => font)));
      }
    } else {
      for (const { cacheKey, font } of resolvedFonts) {
        if (hasLoadedResource(cacheKey, cache.loadedFontKeys, cache.loadedFontObjects)) {
          continue;
        }

        markLoadedResource(cacheKey, cache.loadedFontKeys, cache.loadedFontObjects);
        renderer.loadFont(font);
      }
    }
  }

  if (options?.persistentImages && options.persistentImages.length > 0) {
    const resolvedImages = await Promise.all(
      options.persistentImages.map(async (image) => ({
        cacheKey: createImageCacheKey(image),
        image: await resolvePersistentImage(cache, image),
      })),
    );

    for (const { cacheKey, image } of resolvedImages) {
      if (hasLoadedResource(cacheKey, cache.loadedImageKeys, cache.loadedImageObjects)) {
        continue;
      }

      markLoadedResource(cacheKey, cache.loadedImageKeys, cache.loadedImageObjects);

      const maybePromise = renderer.putPersistentImage(image, options.signal);

      if (maybePromise instanceof Promise) {
        tasks.push(maybePromise);
      }
    }
  }

  if (tasks.length > 0) {
    await Promise.all(tasks);
  }
}

export function createImageResponse(
  defaultOptions?: ImageResponseOptionsWithoutRenderer,
): ImageResponseFactory {
  const cache = createResourceCache();

  return function imageResponse(component: ReactNode, options?: ImageResponseOptions) {
    const mergedOptions = mergeOptions(defaultOptions, options);
    const {
      promise: ready,
      reject: rejectReady,
      resolve: resolveReady,
    } = Promise.withResolvers<void>();
    const module = mergedOptions && "module" in mergedOptions ? mergedOptions.module : undefined;

    const stream = new ReadableStream({
      type: "bytes",
      async start(controller) {
        try {
          const imports = await getImports(module);
          const isExternalRenderer = mergedOptions && "renderer" in mergedOptions;
          const renderer = isExternalRenderer
            ? mergedOptions.renderer
            : (cache.renderer ??= new imports.Renderer({
                loadDefaultFonts: mergedOptions?.loadDefaultFonts,
              }));

          if (!isExternalRenderer) {
            await loadRendererResources(renderer, mergedOptions, cache);
          }

          const image = await renderComponent(component, mergedOptions, imports, renderer);

          controller.enqueue(image as ArrayBufferView<ArrayBuffer>);
          controller.close();
          resolveReady();
        } catch (error) {
          controller.error(error);

          const errorHandler = mergedOptions?.onError ?? defaultErrorHandler;
          await errorHandler(error);

          rejectReady(error);
        }
      },
    });
    const headers = new Headers(mergedOptions?.headers);

    if (!headers.get("content-type")) {
      headers.set("content-type", contentTypeMap[mergedOptions?.format ?? defaultFormat]);
    }

    const response = new Response(stream, {
      headers,
      status: mergedOptions?.status,
      statusText: mergedOptions?.statusText,
    });

    return Object.defineProperty(response, "ready", {
      enumerable: false,
      value: ready,
      writable: false,
    }) as ImageResponseResult;
  };
}

let defaultImageResponse: ImageResponseFactory | undefined;

export class ImageResponse extends Response {
  readonly ready: Promise<void>;

  constructor(component: ReactNode, options?: ImageResponseOptions) {
    defaultImageResponse ??= createImageResponse();

    const response = defaultImageResponse(component, options);

    super(response.body, response);
    this.ready = response.ready;
  }
}

export default ImageResponse;
