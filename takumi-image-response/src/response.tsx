import { fetchResources } from "@takumi-rs/helpers";
import { type EmojiType, extractEmojis } from "@takumi-rs/helpers/emoji";
import { type FromJsxOptions, fromJsx } from "@takumi-rs/helpers/jsx";
import type { ReactNode } from "react";
import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";
import {
  createFontCacheKey,
  createImageCacheKey,
  resolveFont,
  resolvePersistentImage,
  type ImageResponseFont,
  type ImageResponsePersistentImage,
} from "./cache";
import { getImports, type Imports } from "./import";

let renderer: napi.Renderer | wasm.Renderer | undefined;
let rendererPromise: Promise<napi.Renderer | wasm.Renderer> | undefined;

const defaultFormat = "webp";

const loadedFontKeys = new Set<string>();
const loadedImageKeys = new Set<string>();
const loadedFontObjects = new WeakSet<object>();
const loadedImageObjects = new WeakSet<object>();

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}

type RenderOptions = napi.RenderOptions | wasm.RenderOptions;
type ConstructRendererOptions = {
  fonts?: ImageResponseFont[];
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
  };

type ImageResponseOptionsWithoutRenderer = Omit<ImageResponseOptionsWithRenderer, "renderer"> &
  ConstructRendererOptions;

export type ImageResponseOptions =
  | ImageResponseOptionsWithRenderer
  | ImageResponseOptionsWithoutRenderer;

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

async function loadRendererResources(
  activeRenderer: napi.Renderer | wasm.Renderer,
  options: ImageResponseOptions | undefined,
) {
  const tasks: Promise<unknown>[] = [];

  if (hasManagedRendererOptions(options) && options?.fonts && options.fonts.length > 0) {
    const resolvedFonts = await Promise.all(
      options.fonts.map(async (font) => ({
        cacheKey: createFontCacheKey(font),
        font: await resolveFont(font),
      })),
    );

    if ("loadFonts" in activeRenderer) {
      const filteredFonts = resolvedFonts.filter(({ cacheKey }) => {
        if (hasLoadedResource(cacheKey, loadedFontKeys, loadedFontObjects)) {
          return false;
        }

        markLoadedResource(cacheKey, loadedFontKeys, loadedFontObjects);
        return true;
      });

      if (filteredFonts.length > 0) {
        tasks.push(activeRenderer.loadFonts(filteredFonts.map(({ font }) => font)));
      }
    } else {
      for (const { cacheKey, font } of resolvedFonts) {
        if (hasLoadedResource(cacheKey, loadedFontKeys, loadedFontObjects)) {
          continue;
        }

        markLoadedResource(cacheKey, loadedFontKeys, loadedFontObjects);
        activeRenderer.loadFont(font);
      }
    }
  }

  if (
    hasManagedRendererOptions(options) &&
    options?.persistentImages &&
    options.persistentImages.length > 0
  ) {
    const resolvedImages = await Promise.all(
      options.persistentImages.map(async (image) => ({
        cacheKey: createImageCacheKey(image),
        image: await resolvePersistentImage(image),
      })),
    );

    for (const { cacheKey, image } of resolvedImages) {
      if (hasLoadedResource(cacheKey, loadedImageKeys, loadedImageObjects)) {
        continue;
      }

      markLoadedResource(cacheKey, loadedImageKeys, loadedImageObjects);

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

async function getRenderer(options: ImageResponseOptions | undefined, imports: Imports) {
  if (options && "renderer" in options) {
    return options.renderer;
  }

  if (!rendererPromise) {
    rendererPromise = Promise.resolve(
      new imports.Renderer(
        !hasManagedRendererOptions(options) || options.loadDefaultFonts === undefined
          ? undefined
          : {
              loadDefaultFonts: options.loadDefaultFonts,
            },
      ),
    )
      .then((createdRenderer) => {
        renderer = createdRenderer;
        return createdRenderer;
      })
      .catch((error) => {
        rendererPromise = undefined;
        throw error;
      });
  }

  const activeRenderer = await rendererPromise;

  await loadRendererResources(activeRenderer, options);

  return activeRenderer;
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

function createStream(component: ReactNode, options?: ImageResponseOptions) {
  return new ReadableStream({
    type: "bytes",
    async start(controller) {
      try {
        const imports = await getImports(
          options !== undefined && "module" in options ? options.module : undefined,
        );
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

        const mergedOptions = {
          ...options,
          fetchedResources,
          stylesheets: [...(options?.stylesheets ?? []), ...stylesheets],
          format: options?.format ?? defaultFormat,
        };

        const image = await renderer.render(node, mergedOptions, options?.signal);

        controller.enqueue(image as ArrayBufferView<ArrayBuffer>);
        controller.close();
      } catch (error) {
        controller.error(error);
      }
    },
  });
}

const contentTypeMapping = {
  webp: "image/webp",
  png: "image/png",
  jpeg: "image/jpeg",
  raw: "application/octet-stream",
};

export class ImageResponse extends Response {
  constructor(component: ReactNode, options?: ImageResponseOptions) {
    const stream = createStream(component, options);
    const headers = new Headers(options?.headers);

    if (!headers.get("content-type")) {
      headers.set("content-type", contentTypeMapping[options?.format ?? defaultFormat]);
    }

    super(stream, {
      status: options?.status,
      statusText: options?.statusText,
      headers,
    });
  }
}

export default ImageResponse;
