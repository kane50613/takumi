import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";
import { extractEmojis, type EmojiType } from "@takumi-rs/helpers/emoji";
import { prepareImages, type FetchOptions, type ImageFetchCache } from "@takumi-rs/helpers";
import { fromJsx, type FromJsxOptions } from "@takumi-rs/helpers/jsx";
import { getImports } from "./import";
import type { ReactNode } from "react";
import type { Node, ReactElementLike } from "@takumi-rs/helpers";
import { fromHtml } from "@takumi-rs/helpers/html";

type Renderer = napi.Renderer | wasm.Renderer;
type ImageLoader = napi.ImageLoader | wasm.ImageLoader;

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
      /** Decode-cache default for every image this render; a source's own `cache` wins. */
      cache?: NonNullable<ImageLoader["cache"]>;
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

/**
 * `Omit` that distributes over unions, so the backends' `format`/`quality`/`lossless`
 * tagged union survives instead of collapsing into one flat member.
 */
type DistributiveOmit<T, K extends PropertyKey> = T extends unknown ? Omit<T, K> : never;

/** The wrapper-level CSS inputs, replacing the backends' `stylesheets` field. */
type CssOptions = {
  /** CSS to apply before rendering, one string or a list cascading in order. */
  css?: string | readonly string[];
  /**
   * CSS stylesheets to apply before rendering.
   * @deprecated Use `css` instead.
   */
  stylesheets?: string[];
};

type InnerRenderOptions = DistributiveOmit<
  napi.RenderOptions | wasm.RenderOptions,
  "images" | "css"
> &
  CssOptions;
type InnerSvgRenderOptions = DistributiveOmit<
  napi.SvgRenderOptions | wasm.SvgRenderOptions,
  "images" | "css"
> &
  CssOptions;
type InnerRenderAnimationOptions = DistributiveOmit<
  napi.RenderAnimationOptions | wasm.RenderAnimationOptions,
  "images" | "css"
> &
  CssOptions;

export type RenderOptions = Managed<InnerRenderOptions>;
export type RenderSvgOptions = Managed<InnerSvgRenderOptions>;

/** A single animation scene whose content is any renderable input. */
export type AnimationScene = Omit<napi.AnimationScene, "node"> & {
  /** The content to render for this scene: JSX, an HTML string, or a node tree. */
  node: RenderInput;
};

export type RenderAnimationOptions = Managed<
  DistributiveOmit<InnerRenderAnimationOptions, "scenes"> & {
    /** The scenes to render sequentially. */
    scenes: AnimationScene[];
  }
>;

/** The subset of options the shared pipeline reads, across every entry point. */
type PipelineOptions = Partial<SharedRenderExtras> &
  ManagedRendererOptions & {
    css?: string | readonly string[];
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
async function collectImages(
  node: Node | Node[],
  options?: PipelineOptions,
): Promise<ImageLoader[]> {
  const images = options?.images;
  const { sources, fetchCache, fetch, timeout, maxBytes, allowUrl, cache } = Array.isArray(images)
    ? { sources: images }
    : (images ?? {});

  const prepared = await prepareImages({
    node,
    sources,
    fetchCache,
    fetch,
    timeout,
    maxBytes,
    allowUrl,
    signal: options?.signal,
  });

  return cache
    ? prepared.map((image) => ({
        ...image,
        cache: ("cache" in image ? image.cache : undefined) ?? cache,
      }))
    : prepared;
}

function mergeCss(options: PipelineOptions | undefined, extra: string[]): string[] {
  if (options?.css !== undefined && options?.stylesheets !== undefined) {
    throw new Error("pass either `css` or `stylesheets`, not both");
  }

  const own =
    options?.css !== undefined
      ? typeof options.css === "string"
        ? [options.css]
        : options.css
      : (options?.stylesheets ?? []);

  return [...own, ...extra];
}

/**
 * Renders a React element, HTML string, or Takumi node tree into an image.
 *
 * This function automatically detects the best renderer for your environment (native Rust on Node.js,
 * WASM on Edge/Workers) and handles fetching fonts and images, and emoji extraction.
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
  // abort that happened during the async font and image loading before the blocking call.
  options?.signal?.throwIfAborted();

  const { css: _, stylesheets: _alias, ...forward } = options ?? {};

  return renderer.render(node, {
    ...forward,
    images,
    css: mergeCss(options, stylesheets),
  });
}

/**
 * Renders a React element, HTML string, or Takumi node tree into a vector SVG
 * document string.
 *
 * Same input handling and image pipeline as {@link render}, but emits real SVG
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

  const { css: _, stylesheets: _alias, ...forward } = options ?? {};

  return renderer.renderSvg(node, {
    ...forward,
    images,
    css: mergeCss(options, stylesheets),
  });
}

/**
 * Renders a sequence of scenes into an animated image (WebP / APNG / GIF).
 *
 * Each scene's content goes through the same input handling and image pipeline
 * as {@link render}; images are fetched once across all scenes.
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

  const { css: _, stylesheets: _alias, ...forward } = options;

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
  const css = mergeCss(
    options,
    scenes.flatMap((scene) => scene.stylesheets),
  );

  options.signal?.throwIfAborted();

  return renderer.renderAnimation({
    ...forward,
    scenes: scenes.map(({ node, durationMs }) => ({ node, durationMs })),
    images,
    css,
  });
}
