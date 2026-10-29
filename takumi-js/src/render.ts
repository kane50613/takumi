import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";
import { extractEmojis, type EmojiType } from "@takumi-rs/helpers/emoji";
import { type CssInput, type FetchOptions, type ImageFetchCache } from "@takumi-rs/helpers";
import { fromJsx, type FromJsxOptions } from "@takumi-rs/helpers/jsx";
import { defaultRenderer, type Renderer } from "./backend/renderer";
import type { ReactNode } from "react";
import type { Node, ReactElementLike } from "@takumi-rs/helpers";
import { fromHtml } from "@takumi-rs/helpers/html";

type ImageLoader = napi.ImageLoader | wasm.ImageLoader;

/** Pre-fetched images, or options for fetching and caching remote images and emoji. */
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
   * Where emoji images come from. `"from-font"` draws them from the loaded fonts.
   * @default "twemoji"
   */
  emoji?: EmojiType | "from-font";
};

type ManagedRendererOptions = {
  /** The WebAssembly module for the renderer, resolved automatically when omitted. */
  module?: wasm.InitInput | Promise<wasm.InitInput> | { default: wasm.InitInput };
};

/** `TInner` plus a `renderer` to use, or a WASM `module` for the one Takumi resolves. */
type Managed<TInner> =
  | (TInner & SharedRenderExtras)
  | (TInner & Omit<SharedRenderExtras, "renderer"> & ManagedRendererOptions);

/** `Omit` that distributes over unions, keeping the `format` tagged union intact. */
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

/** Turns an input into what a renderer takes, splitting out its emoji unless the renderer's fonts draw them. */
async function resolveContent(
  element: RenderInput,
  options?: PipelineOptions,
): Promise<{ node: Node | string; css: string[] }> {
  if (typeof element === "string" && options?.emoji === "from-font") {
    return { node: element, css: [] };
  }

  const { node: originalNode, css } = await transformElement(element, options);
  const emojiType = options?.emoji ?? "twemoji";
  const node = emojiType !== "from-font" ? extractEmojis(originalNode, emojiType) : originalNode;

  return { node, css };
}

/** A render's options without the ones this pipeline reads itself, with `css` merged. */
function bindingOptions(
  options: PipelineOptions | undefined,
  css: CssInput[],
): NonNullable<Parameters<Renderer["render"]>[1]> {
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

/** Readies a single scene for its render, fetching what the renderer reads after the cascade. */
async function prepareScene<T extends PipelineOptions>(element: RenderInput, options?: T) {
  options?.signal?.throwIfAborted();

  const renderer = await resolveRenderer(options);
  const { node, css } = await resolveContent(element, options);
  const layout = bindingOptions(options, mergeCss(options, css));
  const prepared = await renderer.prepare(node, {
    ...layout,
    images: options?.images,
    signal: options?.signal,
  });

  return { renderer, prepared, output: { ...layout, signal: options?.signal } };
}

/**
 * Renders a React element, HTML string, or node tree into an image, on the native renderer in
 * Node.js and WebAssembly elsewhere, fetching its fonts, images, and emoji.
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
 */
export async function render(
  element: RenderInput,
  options?: RenderOptions,
): Promise<Buffer<ArrayBuffer> | Uint8Array<ArrayBuffer>> {
  const scene = await prepareScene(element, options);

  return scene.renderer.render(scene.prepared, scene.output);
}

/**
 * Lays out a React element, HTML string, or node tree as {@link render} would, without painting it.
 *
 * @example
 * ```tsx
 * import { measure } from "takumi-js";
 *
 * const { width, height } = await measure(<span tw="text-xl">Hello World</span>);
 * ```
 */
export async function measure(
  element: RenderInput,
  options?: RenderOptions,
): Promise<napi.MeasuredNode> {
  const scene = await prepareScene(element, options);

  return scene.renderer.measure(scene.prepared, { signal: options?.signal });
}

/**
 * Renders a React element, HTML string, or node tree as {@link render} would, into an SVG document.
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
 */
export async function renderSvg(element: RenderInput, options?: RenderSvgOptions): Promise<string> {
  const scene = await prepareScene(element, options);

  return scene.renderer.renderSvg(scene.prepared, { signal: options?.signal });
}

/**
 * Renders scenes in sequence into an animated WebP, APNG, or GIF, fetching their images once.
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
 */
export async function renderAnimation(
  options: RenderAnimationOptions,
): Promise<Buffer<ArrayBuffer> | Uint8Array<ArrayBuffer>> {
  options.signal?.throwIfAborted();

  const renderer = await resolveRenderer(options);
  const scenes = await Promise.all(
    options.scenes.map(async (scene) => {
      const { node, css } = await resolveContent(scene.node, options);
      return { node, durationMs: scene.durationMs, css };
    }),
  );
  const { css: _, stylesheets: _alias, ...forward } = options;
  const prepared = await renderer.prepareAnimation({
    ...forward,
    css: mergeCss(
      options,
      scenes.flatMap((scene) => scene.css),
    ),
    scenes: scenes.map(({ node, durationMs }) => ({ node, durationMs })),
  });

  return renderer.renderAnimation(prepared, { signal: options.signal });
}
