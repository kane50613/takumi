import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";
import { emojiImageFont, extractEmojis, type EmojiType } from "@takumi-rs/helpers/emoji";
import { type ImageFont, type ImageFontLoader, loadImageFonts } from "@takumi-rs/helpers/renderer";
import {
  prepareImages,
  type CssInput,
  type FetchOptions,
  type ImageFetchCache,
} from "@takumi-rs/helpers";
import { fromJsx, type FromJsxOptions } from "@takumi-rs/helpers/jsx";
import { defaultRenderer, type Renderer } from "./backend/renderer";
import type { ReactNode } from "react";
import type { Node, ReactElementLike } from "@takumi-rs/helpers";
import { fromHtml } from "@takumi-rs/helpers/html";

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
      /** Throw on any fetch failure; if `false`, failed images are left out of the render. @default true */
      throwOnError?: boolean;
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
  css?: CssInput | readonly CssInput[];
  /**
   * CSS stylesheets to apply before rendering.
   * @deprecated Use `css` instead. Will be removed in v3.
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
    width?: number;
    height?: number;
    css?: CssInput | readonly CssInput[];
    stylesheets?: string[];
  };

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
      css: [],
    };
  }

  if (typeof element === "string") {
    return fromHtml(element);
  }

  return fromJsx(element, options?.jsx);
}

/** Resolves a caller-supplied renderer or the default provider. */
async function resolveRenderer(options?: PipelineOptions): Promise<Renderer> {
  if (options && "renderer" in options && options.renderer) {
    return options.renderer;
  }

  return defaultRenderer.get(options?.module);
}

/**
 * The image fonts a current renderer draws emoji with: the CDN set `emoji` names, which defaults
 * to Twemoji here, or none for `from-font`.
 */
function emojiImageFonts(
  renderer: Renderer,
  options?: PipelineOptions,
): ImageFontLoader[] | undefined {
  const emoji = options?.emoji ?? "twemoji";

  return isCurrent(renderer) && emoji !== "from-font" ? [emojiImageFont(emoji)] : undefined;
}

/** Whether `renderer` parses HTML, draws image fonts, and reads what a source needs itself, as bindings with `prepare` do. */
function isCurrent(renderer: Renderer): boolean {
  return "prepare" in renderer;
}

/**
 * Turns an input into what `renderer` takes: HTML as is and a node tree with its emoji in place
 * for a current renderer, which draws them with an image font, and a node tree with its emoji
 * split out as images for an older one.
 */
async function resolveContent(
  element: RenderInput,
  renderer: Renderer,
  options?: PipelineOptions,
): Promise<{ node: Node | string; css: string[] }> {
  if (isCurrent(renderer) && typeof element === "string") {
    return { node: element, css: [] };
  }

  const { node, css } = await transformElement(element, options);
  const emojiType = options?.emoji ?? "twemoji";

  return {
    node: isCurrent(renderer) || emojiType === "from-font" ? node : extractEmojis(node, emojiType),
    css,
  };
}

/** A render's options without the ones this pipeline reads itself, with `css` merged. */
function bindingOptions(options: PipelineOptions | undefined, css: CssInput[]): NeedsOptions {
  const {
    renderer: _renderer,
    jsx: _jsx,
    module: _module,
    emoji: _emoji,
    images: _images,
    signal: _signal,
    css: _css,
    stylesheets: _stylesheets,
    ...rest
  } = options ?? {};

  return { ...rest, css };
}

/** The binding options `needs` reads, the same ones a render passes on. */
type NeedsOptions = Parameters<Extract<Renderer, { needs: unknown }>["needs"]>[1];

/**
 * Fetches the images `renderer` reports `nodes` need under the binding options `layout`, via
 * {@link prepareImages}, and loads the clusters each of `imageFonts` lacks. A renderer without
 * `needs`, from an older binding, gets the images from the node trees instead.
 */
async function collectImages(
  renderer: Renderer,
  nodes: (Node | string)[],
  layout: NeedsOptions,
  options?: PipelineOptions,
  imageFonts?: ImageFontLoader[],
): Promise<{ images: ImageLoader[]; imageFonts?: ImageFont[] }> {
  const images = options?.images;
  const { sources, fetchCache, fetch, timeout, maxBytes, allowUrl, cache, throwOnError } =
    Array.isArray(images) ? { sources: images } : (images ?? {});
  const signal = options?.signal;
  const needsLayout = imageFonts
    ? { ...layout, imageFonts: imageFonts.map(({ name }) => ({ name })) }
    : layout;

  const needs =
    "needs" in renderer
      ? await Promise.all(nodes.map((node) => renderer.needs(node, needsLayout)))
      : undefined;
  const prepared = await prepareImages({
    urls: needs?.flatMap((scene) => scene.images),
    node: needs ? undefined : nodes.filter((node) => typeof node !== "string"),
    sources,
    fetchCache,
    fetch,
    timeout,
    maxBytes,
    allowUrl,
    throwOnError,
    signal,
  });
  const loadedImageFonts =
    imageFonts &&
    needs &&
    (await loadImageFonts(
      imageFonts,
      needs.flatMap((scene) => scene.glyphs),
      { fetchCache, fetch, timeout, maxBytes, allowUrl, signal },
    ));

  return {
    images: cache
      ? prepared.map((image) => ({
          ...image,
          cache: ("cache" in image ? image.cache : undefined) ?? cache,
        }))
      : prepared,
    imageFonts: loadedImageFonts,
  };
}

let warnedStylesheets = false;

function warnStylesheetsDeprecated(): void {
  if (warnedStylesheets) return;
  warnedStylesheets = true;
  console.warn("takumi: the `stylesheets` option is deprecated, use `css` instead.");
}

/** Narrows the `css` option, which takes one entry or a list of them. */
function isCssList(css: CssInput | readonly CssInput[]): css is readonly CssInput[] {
  return Array.isArray(css);
}

function mergeCss(options: PipelineOptions | undefined, extra: string[]): CssInput[] {
  if (options?.css !== undefined && options?.stylesheets !== undefined) {
    throw new Error("pass either `css` or `stylesheets`, not both");
  }

  if (options?.stylesheets !== undefined) {
    warnStylesheetsDeprecated();
  }

  const own =
    options?.css !== undefined
      ? isCssList(options.css)
        ? options.css
        : [options.css]
      : (options?.stylesheets ?? []);

  return [...own, ...extra];
}

/**
 * Readies a single scene for its render: a current renderer prepares it, fetching what it reads
 * after the cascade, and an older one gets the node tree with its images fetched.
 */
async function prepareScene<T extends PipelineOptions>(element: RenderInput, options?: T) {
  options?.signal?.throwIfAborted();

  const renderer = await resolveRenderer(options);
  const { node, css } = await resolveContent(element, renderer, options);
  const sceneCss = mergeCss(options, css);

  if (isCurrent(renderer)) {
    const layout = bindingOptions(options, sceneCss);
    const prepared = await renderer.prepare(node, {
      ...layout,
      imageFonts: emojiImageFonts(renderer, options),
      images: options?.images,
      signal: options?.signal,
    });

    return { renderer, prepared, output: { ...layout, signal: options?.signal } };
  }

  const { css: _, stylesheets: _alias, ...forward } = options ?? {};
  const { images } = await collectImages(
    renderer,
    [node],
    bindingOptions(options, sceneCss),
    options,
  );

  // The WASM renderer is synchronous and ignores the signal argument, so honor an
  // abort that happened during the async font and image loading before the blocking call.
  options?.signal?.throwIfAborted();

  return { renderer, node, options: { ...forward, images, css: sceneCss } };
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
export async function render(
  element: RenderInput,
  options?: RenderOptions,
): Promise<Buffer<ArrayBuffer> | Uint8Array<ArrayBuffer>> {
  const scene = await prepareScene(element, options);

  return scene.prepared
    ? scene.renderer.render(scene.prepared, scene.output)
    : scene.renderer.render(scene.node, scene.options);
}

/**
 * Lays out a React element, HTML string, or Takumi node tree without painting it.
 *
 * Same input handling, CSS, emoji, and image pipeline as {@link render}, on the same default
 * renderer, so fonts registered for rendering measure too.
 *
 * @example
 * ```tsx
 * import { measure } from "takumi-js";
 *
 * const { width, height } = await measure(<span tw="text-xl">Hello World</span>);
 * ```
 *
 * @returns A promise that resolves to the measured node tree.
 */
export async function measure(
  element: RenderInput,
  options?: RenderOptions,
): Promise<napi.MeasuredNode> {
  const scene = await prepareScene(element, options);

  return scene.prepared
    ? scene.renderer.measure(scene.prepared, { signal: options?.signal })
    : scene.renderer.measure(scene.node, scene.options);
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
  const scene = await prepareScene(element, options);

  return scene.prepared
    ? scene.renderer.renderSvg(scene.prepared, { signal: options?.signal })
    : scene.renderer.renderSvg(scene.node, scene.options);
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
export async function renderAnimation(
  options: RenderAnimationOptions,
): Promise<Buffer<ArrayBuffer> | Uint8Array<ArrayBuffer>> {
  options.signal?.throwIfAborted();

  const { css: _, stylesheets: _alias, emoji: _emoji, ...forward } = options;

  const renderer = await resolveRenderer(options);
  const scenes = await Promise.all(
    options.scenes.map(async (scene) => {
      const { node, css } = await resolveContent(scene.node, renderer, options);
      return { node, durationMs: scene.durationMs, css };
    }),
  );

  const css = mergeCss(
    options,
    scenes.flatMap((scene) => scene.css),
  );
  const { images, imageFonts } = await collectImages(
    renderer,
    scenes.map((scene) => scene.node),
    {
      width: options.width,
      height: options.height,
      devicePixelRatio: options.devicePixelRatio,
      fontFamilies: options.fontFamilies,
      lang: options.lang,
      keyframes: options.keyframes,
      css,
    },
    options,
    emojiImageFonts(renderer, options),
  );

  options.signal?.throwIfAborted();

  return renderer.renderAnimation({
    ...forward,
    scenes: scenes.map(({ node, durationMs }) => ({ node, durationMs })),
    images,
    css,
    ...(imageFonts && { imageFonts }),
  });
}
