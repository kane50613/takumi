import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";
import { extractEmojis, type EmojiType } from "@takumi-rs/helpers/emoji";
import { extractResourceUrls, fetchResources } from "@takumi-rs/helpers";
import { fromJsx, type FromJsxOptions } from "@takumi-rs/helpers/jsx";
import { getImports } from "./import";
import type { ReactNode } from "react";
import type { FetchResourcesOptions, Node, ReactElementLike } from "@takumi-rs/helpers";
import { fromHtml } from "@takumi-rs/helpers/html";

type Renderer = napi.Renderer | wasm.Renderer;
type ImageLoader = napi.ImageLoader | wasm.ImageLoader;

/** The managed-renderer plumbing shared by every entry point. */
type SharedRenderExtras = {
  renderer: Renderer;
  signal?: AbortSignal;
  jsx?: FromJsxOptions;
  resourcesOptions?: FetchResourcesOptions;
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

type InnerRenderOptions = napi.RenderOptions | wasm.RenderOptions;
type InnerSvgRenderOptions = napi.SvgRenderOptions | wasm.SvgRenderOptions;
type InnerRenderAnimationOptions = napi.RenderAnimationOptions | wasm.RenderAnimationOptions;

export type RenderOptions = Managed<InnerRenderOptions>;
export type RenderSvgOptions = Managed<InnerSvgRenderOptions>;

/** A single animation scene whose content is any renderable input. */
export type RenderAnimationScene = Omit<napi.AnimationSceneSource, "node"> & {
  /** The content to render for this scene: JSX, an HTML string, or a node tree. */
  node: RenderInput;
};

export type RenderAnimationOptions = Managed<
  Omit<InnerRenderAnimationOptions, "scenes"> & {
    /** The scenes to render sequentially. */
    scenes: RenderAnimationScene[];
  }
>;

/** The subset of options the shared pipeline reads, across every entry point. */
type PipelineOptions = Partial<SharedRenderExtras> &
  ManagedRendererOptions & {
    images?: ImageLoader[];
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

/** Merges caller-supplied images with the resources fetched from the nodes. */
async function collectImages(nodes: Node[], options?: PipelineOptions): Promise<ImageLoader[]> {
  const providedImages = new Map<string, ImageLoader>();

  for (const image of options?.images ?? []) {
    providedImages.set(image.src, image);
  }

  const urls = [...new Set(nodes.flatMap((node) => extractResourceUrls(node)))].filter(
    (url) => !providedImages.has(url),
  );
  const fetched = await fetchResources(urls, options?.resourcesOptions);

  return [...providedImages.values(), ...fetched];
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
  const images = await collectImages([node], options);

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
  const images = await collectImages([node], options);

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
