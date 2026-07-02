import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";
import { extractEmojis, type EmojiType } from "@takumi-rs/helpers/emoji";
import { fetchOk, type FetchOptions } from "@takumi-rs/helpers";
import { fromJsx, type FromJsxOptions } from "@takumi-rs/helpers/jsx";
import { getImports } from "./import";
import type { CSSProperties, ReactNode } from "react";
import type { Node, ReactElementLike } from "@takumi-rs/helpers";
import { fromHtml } from "@takumi-rs/helpers/html";

type Renderer = napi.Renderer | wasm.Renderer;
type ImageLoader = napi.ImageLoader | wasm.ImageLoader;

/**
 * A cache of image fetches keyed by URL. Sharing one across renders deduplicates concurrent
 * requests for the same URL (single-flight) and reuses their bytes. Any object with `Map`-like
 * `get`/`set`/`delete` works, so LRU/TTL policies can be plugged in.
 */
export type ImageFetchCache = Pick<Map<string, Promise<ArrayBuffer>>, "get" | "set" | "delete">;

const defaultFetchTimeout = 5000;
const cssUrlPattern = /url\(\s*(['"]?)(.*?)\1\s*\)/g;

function isRemoteUrl(value: string): boolean {
  return value.startsWith("https://") || value.startsWith("http://");
}

function collectCssUrls(value: unknown, urls: Set<string>) {
  if (typeof value === "string") {
    for (const match of value.matchAll(cssUrlPattern)) {
      const url = match[2]?.trim();
      if (url && isRemoteUrl(url)) {
        urls.add(url);
      }
    }
  } else if (Array.isArray(value)) {
    for (const item of value) {
      collectCssUrls(item, urls);
    }
  }
}

/** Every remote image URL a node tree references: `<img src>`, `backgroundImage`, `maskImage`. */
function extractResourceUrls(node: Node): string[] {
  const urls = new Set<string>();

  const visit = (current: Node) => {
    const collectStyleUrls = (style: CSSProperties | undefined) => {
      if (!style) {
        return;
      }

      collectCssUrls(style.backgroundImage, urls);
      collectCssUrls(style.maskImage, urls);
    };

    collectStyleUrls(current.style);
    collectStyleUrls(current.preset);
    collectCssUrls(current.tw, urls);

    if (current.type === "image") {
      if (typeof current.src === "string" && isRemoteUrl(current.src)) {
        urls.add(current.src);
      }
      return;
    }

    if (current.type === "container") {
      for (const child of current.children ?? []) {
        visit(child);
      }
    }
  };

  visit(node);
  return [...urls];
}

/** Fetches a URL's bytes, coalescing concurrent requests for the same URL through `cache`. A
 * rejected fetch is evicted so a later call can retry instead of replaying the failure. */
function fetchImageData(
  url: string,
  options: FetchOptions,
  fetchCache?: ImageFetchCache,
): Promise<ArrayBuffer> {
  const cached = fetchCache?.get(url);
  if (cached) {
    return cached;
  }

  const promise = fetchOk(url, options)
    .then((response) => response.arrayBuffer())
    .catch((error) => {
      fetchCache?.delete(url);
      throw error;
    });

  fetchCache?.set(url, promise);
  return promise;
}

export type PrepareImagesOptions = FetchOptions & {
  /** The node tree(s) whose remote images to fetch. */
  node: Node | Node[];
  /** Pre-fetched entries; their URLs are not re-fetched. */
  sources?: ImageLoader[];
  /** Single-flight byte cache shared across renders. */
  fetchCache?: ImageFetchCache;
  /** Throw on any fetch failure; if `false`, failed URLs are dropped. @default true */
  throwOnError?: boolean;
};

/**
 * Collects every remote image a node tree references, fetches the ones not already in `sources`,
 * and returns them as `images` entries ready to hand to a renderer.
 */
export async function prepareImages({
  node,
  sources = [],
  fetchCache,
  fetch,
  timeout = defaultFetchTimeout,
  throwOnError = true,
}: PrepareImagesOptions): Promise<ImageLoader[]> {
  const nodes = Array.isArray(node) ? node : [node];
  const provided = new Map<string, ImageLoader>();

  for (const image of sources) {
    provided.set(image.src, image);
  }

  const urls = [...new Set(nodes.flatMap(extractResourceUrls))].filter((url) => !provided.has(url));
  const fetchOptions: FetchOptions = { fetch, timeout };

  const tasks = urls.map(async (src) => ({
    src,
    data: await fetchImageData(src, fetchOptions, fetchCache),
  }));
  const fetched = throwOnError
    ? await Promise.all(tasks)
    : (await Promise.allSettled(tasks))
        .filter((result) => result.status === "fulfilled")
        .map((result) => result.value);

  return [...provided.values(), ...fetched];
}

/**
 * Images for a render: pre-fetched entries, or a group that also controls how remote images
 * (and emoji glyphs) are fetched and cached.
 */
export type ImagesInput =
  | ImageLoader[]
  | (FetchOptions & {
      /** Pre-fetched entries, same as the array form. */
      sources?: ImageLoader[];
      /** Single-flight byte cache shared across renders. */
      fetchCache?: ImageFetchCache;
    });

/** The managed-renderer plumbing shared by every entry point. */
type SharedRenderExtras = {
  renderer: Renderer;
  signal?: AbortSignal;
  jsx?: FromJsxOptions;
  images?: ImagesInput;
  /**
   * @description The emoji provider to use when rendering emojis. If set to `"from-font"`, the renderer will attempt to source emoji glyphs from the loaded fonts.
   * @default "twemoji"
   */
  emoji?: EmojiType | "from-font";
};

type ManagedRendererOptions = {
  /**
   * @description The WebAssembly module to use for the renderer. If not provided, the default resolving strategy will be used.
   */
  module?: wasm.InitInput | Promise<wasm.InitInput> | { default: wasm.InitInput };
};

/**
 * Adds the managed-renderer plumbing to a set of inner options. Either bring your
 * own `renderer`, or let Takumi resolve one (optionally pointing at a WASM
 * `module`).
 */
type Managed<TInner> =
  | (TInner & SharedRenderExtras)
  | (TInner & Omit<SharedRenderExtras, "renderer"> & ManagedRendererOptions);

type InnerRenderOptions = Omit<napi.RenderOptions | wasm.RenderOptions, "images">;
type InnerSvgRenderOptions = Omit<napi.SvgRenderOptions | wasm.SvgRenderOptions, "images">;
type InnerRenderAnimationOptions = Omit<
  napi.RenderAnimationOptions | wasm.RenderAnimationOptions,
  "images"
>;

export type RenderOptions = Managed<InnerRenderOptions>;
export type RenderSvgOptions = Managed<InnerSvgRenderOptions>;

/** A single animation scene whose content is any renderable input. */
export type AnimationScene = Omit<napi.AnimationScene, "node"> & {
  /** The content to render for this scene: JSX, an HTML string, or a node tree. */
  node: RenderInput;
};

export type RenderAnimationOptions = Managed<
  Omit<InnerRenderAnimationOptions, "scenes"> & {
    /** The scenes to render sequentially. */
    scenes: AnimationScene[];
  }
>;

/** The subset of options the shared pipeline reads, across every entry point. */
type PipelineOptions = Partial<SharedRenderExtras> &
  ManagedRendererOptions & {
    stylesheets?: string[];
  };

let globalRenderer: Renderer | undefined;

export type RenderInput = ReactNode | ReactElementLike | Node | string;

function isTakumiNode(element: unknown): element is Node {
  if (typeof element !== "object" || element === null || !("type" in element)) {
    return false;
  }

  return element.type === "container" || element.type === "text" || element.type === "image";
}

async function transformElement(element: RenderInput, options?: PipelineOptions) {
  if (isTakumiNode(element)) {
    return {
      node: element,
      stylesheets: [],
    };
  }

  if (typeof element === "string") {
    return fromHtml(element);
  }

  return fromJsx(element, options?.jsx);
}

/** Resolves the renderer to use: a caller-supplied one, or the shared global. */
async function resolveRenderer(options?: PipelineOptions): Promise<Renderer> {
  if (options && "renderer" in options && options.renderer) {
    return options.renderer;
  }

  const imports = await getImports(options?.module);
  return (globalRenderer ??= new imports.Renderer());
}

/** Transforms an input into a node tree and extracts its emojis. */
async function resolveContent(element: RenderInput, options?: PipelineOptions) {
  const { node: originalNode, stylesheets } = await transformElement(element, options);
  const emojiType = options?.emoji ?? "twemoji";
  const node = emojiType !== "from-font" ? extractEmojis(originalNode, emojiType) : originalNode;

  return { node, stylesheets };
}

/** Resolves the render's `images` option into concrete entries via {@link prepareImages}. */
function collectImages(node: Node | Node[], options?: PipelineOptions): Promise<ImageLoader[]> {
  const images = options?.images;
  const { sources, fetchCache, fetch, timeout } = Array.isArray(images)
    ? { sources: images }
    : (images ?? {});

  return prepareImages({ node, sources, fetchCache, fetch, timeout });
}

function mergeStylesheets(options: PipelineOptions | undefined, extra: string[]): string[] {
  return [...(options?.stylesheets ?? []), ...extra];
}

/**
 * Renders a React element, HTML string, or Takumi node tree into an image.
 *
 * This function automatically detects the best renderer for your environment (native Rust on Node.js,
 * WASM on Edge/Workers) and handles resource fetching (fonts, images) and emoji extraction.
 *
 * @example
 * ```tsx
 * import { render } from "takumi-js";
 *
 * const buffer = await render(
 *   <div tw="bg-blue-500 text-white p-4">Hello World</div>,
 *   { width: 1200, height: 630 }
 * );
 * ```
 *
 * @param element - The content to render. Can be a JSX element (React-like), an HTML string, or a pre-constructed node tree.
 * @param options - Configuration for rendering, including dimensions, format, fonts, and more.
 * @returns A promise that resolves to the rendered image data (Buffer/Uint8Array).
 */
export async function render(element: RenderInput, options?: RenderOptions) {
  options?.signal?.throwIfAborted();

  const renderer = await resolveRenderer(options);
  const { node, stylesheets } = await resolveContent(element, options);
  const images = await collectImages(node, options);

  // The WASM renderer is synchronous and ignores the signal argument, so honor an
  // abort that happened during the async font/resource loading before the blocking call.
  options?.signal?.throwIfAborted();

  return renderer.render(node, {
    ...options,
    images,
    stylesheets: mergeStylesheets(options, stylesheets),
  });
}

/**
 * Renders a React element, HTML string, or Takumi node tree into a vector SVG
 * document string.
 *
 * Same input handling and resource pipeline as {@link render}, but emits real SVG
 * (`<rect>`, `<path>`, gradients, glyph outlines, embedded images) instead of a
 * raster bitmap.
 *
 * @example
 * ```tsx
 * import { renderSvg } from "takumi-js";
 *
 * const svg = await renderSvg(
 *   <div tw="bg-blue-500 text-white p-4">Hello World</div>,
 *   { width: 1200, height: 630 }
 * );
 * ```
 *
 * @returns A promise that resolves to the SVG document string.
 */
export async function renderSvg(element: RenderInput, options?: RenderSvgOptions): Promise<string> {
  options?.signal?.throwIfAborted();

  const renderer = await resolveRenderer(options);
  const { node, stylesheets } = await resolveContent(element, options);
  const images = await collectImages(node, options);

  options?.signal?.throwIfAborted();

  return renderer.renderSvg(node, {
    ...options,
    images,
    stylesheets: mergeStylesheets(options, stylesheets),
  });
}

/**
 * Renders a sequence of scenes into an animated image (WebP / APNG / GIF).
 *
 * Each scene's content goes through the same input handling and resource pipeline
 * as {@link render}; resources are fetched once across all scenes.
 *
 * @example
 * ```tsx
 * import { renderAnimation } from "takumi-js";
 *
 * const webp = await renderAnimation({
 *   width: 600,
 *   height: 400,
 *   fps: 30,
 *   format: "webp",
 *   scenes: [
 *     { node: <div tw="bg-red-500 w-full h-full" />, durationMs: 500 },
 *     { node: <div tw="bg-blue-500 w-full h-full" />, durationMs: 500 },
 *   ],
 * });
 * ```
 *
 * @returns A promise that resolves to the encoded animation (Buffer/Uint8Array).
 */
export async function renderAnimation(options: RenderAnimationOptions) {
  options.signal?.throwIfAborted();

  const renderer = await resolveRenderer(options);
  const scenes = await Promise.all(
    options.scenes.map(async (scene) => {
      const { node, stylesheets } = await resolveContent(scene.node, options);
      return { node, durationMs: scene.durationMs, stylesheets };
    }),
  );

  const images = await collectImages(
    scenes.map((scene) => scene.node),
    options,
  );
  const stylesheets = mergeStylesheets(
    options,
    scenes.flatMap((scene) => scene.stylesheets),
  );

  options.signal?.throwIfAborted();

  return renderer.renderAnimation({
    ...options,
    scenes: scenes.map(({ node, durationMs }) => ({ node, durationMs })),
    images,
    stylesheets,
  });
}
