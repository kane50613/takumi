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
    onError?: (error: unknown) => ReactNode | Promise<ReactNode>;
  };

export type ImageResponseOptionsWithoutRenderer = Omit<
  ImageResponseOptionsWithRenderer,
  "renderer"
> &
  ManagedRendererOptions & {
    onError?: (error: unknown) => ReactNode | Promise<ReactNode>;
  };

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

function hasManagedRendererOptions(
  options: ImageResponseOptions | undefined,
): options is ImageResponseOptionsWithoutRenderer {
  return options === undefined || !("renderer" in options);
}

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

function getContentType(format: RenderOptions["format"] | undefined) {
  switch (format ?? defaultFormat) {
    case "png":
      return "image/png";
    case "jpeg":
      return "image/jpeg";
    case "raw":
      return "application/octet-stream";
    case "webp":
    default:
      return "image/webp";
  }
}

async function renderComponent(
  component: ReactNode,
  options: ImageResponseOptions | undefined,
  imports: Imports,
  getRenderer: (
    options: ImageResponseOptions | undefined,
    imports: Imports,
  ) => Promise<napi.Renderer | wasm.Renderer>,
) {
  const nodePromise = fromJsx(component, options?.jsx).then(async ({ node, stylesheets }) => {
    if (options?.emoji && options.emoji !== "from-font") {
      node = extractEmojis(node, options.emoji);
    }

    const fetchedResources = await extractFetchedResources(node, options, imports);

    return { node, fetchedResources, stylesheets };
  });

  const [renderer, { node, fetchedResources, stylesheets }] = await Promise.all([
    getRenderer(options, imports),
    nodePromise,
  ]);

  const renderOptions = {
    ...options,
    fetchedResources,
    format: options?.format ?? defaultFormat,
    stylesheets: [...(options?.stylesheets ?? []), ...stylesheets],
  };

  return renderer.render(node, renderOptions, options?.signal);
}

function createManagedRendererFactory(
  defaultOptions: ImageResponseOptionsWithoutRenderer | undefined,
  cache: ResourceCache,
) {
  let renderer: napi.Renderer | wasm.Renderer | undefined;

  async function loadRendererResources(
    activeRenderer: napi.Renderer | wasm.Renderer,
    options: ImageResponseOptionsWithoutRenderer | undefined,
  ) {
    const tasks: Promise<unknown>[] = [];

    if (options?.fonts && options.fonts.length > 0) {
      const resolvedFonts = await Promise.all(
        options.fonts.map(async (font) => ({
          cacheKey: createFontCacheKey(font),
          font: await resolveFont(cache, font),
        })),
      );

      if ("loadFonts" in activeRenderer) {
        const filteredFonts = resolvedFonts.filter(({ cacheKey }) => {
          if (hasLoadedResource(cacheKey, cache.loadedFontKeys, cache.loadedFontObjects)) {
            return false;
          }

          markLoadedResource(cacheKey, cache.loadedFontKeys, cache.loadedFontObjects);
          return true;
        });

        if (filteredFonts.length > 0) {
          tasks.push(activeRenderer.loadFonts(filteredFonts.map(({ font }) => font)));
        }
      } else {
        for (const { cacheKey, font } of resolvedFonts) {
          if (hasLoadedResource(cacheKey, cache.loadedFontKeys, cache.loadedFontObjects)) {
            continue;
          }

          markLoadedResource(cacheKey, cache.loadedFontKeys, cache.loadedFontObjects);
          activeRenderer.loadFont(font);
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

        const maybePromise = activeRenderer.putPersistentImage(image, options.signal);

        if (maybePromise instanceof Promise) {
          tasks.push(maybePromise);
        }
      }
    }

    if (tasks.length > 0) {
      await Promise.all(tasks);
    }
  }

  return async function getRenderer(options: ImageResponseOptions | undefined, imports: Imports) {
    if (options && "renderer" in options) {
      return options.renderer;
    }

    renderer ??= new imports.Renderer({
      loadDefaultFonts: options?.loadDefaultFonts ?? defaultOptions?.loadDefaultFonts,
    });

    const managedOptions = hasManagedRendererOptions(options) ? options : defaultOptions;

    await loadRendererResources(renderer, managedOptions);

    return renderer;
  };
}

async function extractFetchedResources(
  node: napi.Node | wasm.Node,
  options: ImageResponseOptions | undefined,
  imports: Imports,
) {
  if (options?.fetchedResources) {
    return options.fetchedResources;
  }

  const urls = imports.extractResourceUrls(node);

  return fetchResources(urls);
}

export function createImageResponse(
  defaultOptions?: ImageResponseOptionsWithoutRenderer,
): ImageResponseFactory {
  const cache = createResourceCache();
  const getRenderer = createManagedRendererFactory(defaultOptions, cache);

  return function imageResponse(component: ReactNode, options?: ImageResponseOptions) {
    const mergedOptions = mergeOptions(defaultOptions, options);
    let resolveReady: (() => void) | undefined;
    let rejectReady: ((reason?: unknown) => void) | undefined;
    const ready = new Promise<void>((resolve, reject) => {
      resolveReady = resolve;
      rejectReady = reject;
    });
    const stream = new ReadableStream({
      type: "bytes",
      async start(controller) {
        const finishSuccess = () => {
          resolveReady?.();
        };
        const finishError = (error: unknown) => {
          rejectReady?.(error);
          controller.error(error);
        };

        try {
          const imports = await getImports(
            mergedOptions !== undefined && "module" in mergedOptions
              ? mergedOptions.module
              : undefined,
          );
          const image = await renderComponent(component, mergedOptions, imports, getRenderer);

          controller.enqueue(image as ArrayBufferView<ArrayBuffer>);
          controller.close();
          finishSuccess();
        } catch (error) {
          if (mergedOptions && "onError" in mergedOptions && mergedOptions.onError) {
            try {
              const fallbackComponent = await mergedOptions.onError(error);
              const imports = await getImports(
                mergedOptions !== undefined && "module" in mergedOptions
                  ? mergedOptions.module
                  : undefined,
              );
              const fallbackImage = await renderComponent(
                fallbackComponent,
                { ...mergedOptions, onError: undefined },
                imports,
                getRenderer,
              );

              controller.enqueue(fallbackImage as ArrayBufferView<ArrayBuffer>);
              controller.close();
              finishSuccess();
              return;
            } catch (fallbackError) {
              finishError(fallbackError);
              return;
            }
          }

          finishError(error);
        }
      },
    });
    const headers = new Headers(mergedOptions?.headers);

    if (!headers.get("content-type")) {
      headers.set("content-type", getContentType(mergedOptions?.format));
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
