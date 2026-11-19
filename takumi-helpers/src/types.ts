import type { CSSProperties } from "react";

/**
 * Value of the `text-fit` longhand: `[ none | grow | shrink ] [ consistent | per-line | per-line-all ]? <percentage>?`.
 *
 * @see https://drafts.csswg.org/css-text-4/#text-fit-property
 */
export type TextFit =
  | "none"
  | "grow"
  | "shrink"
  | "consistent"
  | "per-line"
  | "per-line-all"
  | `${number}%`
  | (string & {});

declare module "react" {
  interface CSSProperties extends Record<`--${string}`, string | number | undefined> {
    textFit?: TextFit;
  }
}

export type NodeAttributes = Record<string, string>;

/** A declaration block, with custom properties allowed. */
export type Declarations = CSSProperties;

/** A style rule written as an object. */
export type StyleRule = {
  selector: string;
  style?: Declarations;
  rules?: StyleRule[];
};

/** An animation written as an object. */
export type AnimationRule = {
  keyframes: string;
  steps: { offset: string; style?: Declarations }[];
};

/** A group of entries gated by a media query. */
export type MediaRule = {
  media: string;
  rules: CssInput[];
};

/** A group of entries gated by a support condition. */
export type SupportsRule = {
  supports: string;
  rules: CssInput[];
};

/** A cascade layer. Without `rules` it declares the layer's order alone. */
export type LayerRule = {
  layer: string;
  rules?: CssInput[];
};

/** One entry of the `css` render option: stylesheet text, or a rule object. */
export type CssInput = string | StyleRule | AnimationRule | MediaRule | SupportsRule | LayerRule;

/** A JSX element from any React-shaped runtime, Preact included. */
export type ReactElementLike = {
  type:
    | string
    | symbol
    | ((props: never) => unknown)
    | (new (props: never) => unknown)
    | ReactElementLike;
  props: unknown;
  $$typeof?: symbol | string;
};

export type NodeMetadata = {
  tagName?: string;
  className?: string;
  id?: string;
  dir?: "ltr" | "rtl";
  lang?: string;
  attributes?: NodeAttributes;
  tw?: string;
  style?: Declarations;
  preset?: Declarations;
};

export type Node = ContainerNode | TextNode | ImageNode;

export type ContainerNode = NodeMetadata & {
  type: "container";
  children?: Node[];
};

export type TextNode = NodeMetadata & {
  type: "text";
  text: string;
};

/** Raw row-major RGBA pixels, rendered without decoding. */
export type RgbaImage = {
  /** The image width in pixels. */
  width: number;
  /** The image height in pixels. */
  height: number;
  /** RGBA bytes, `width * height * 4` long. */
  data: Uint8Array | ArrayBuffer;
  /** The bytes are already alpha-premultiplied, so the premultiply pass is skipped. */
  premultiplied?: boolean;
};

export type ImageNode = NodeMetadata & {
  type: "image";
  src: string | Uint8Array | ArrayBuffer | RgbaImage;
  width?: number;
  height?: number;
};

/**
 * Whether a renderer keeps data for later renders. `"none"` keeps none of it, though a render still
 * uses what the renderer already holds.
 */
export type CacheMode = "auto" | "none";

/** The memory a renderer keeps across renders, one pool of bytes every kind of data shares. */
export interface CacheOptions {
  /**
   * The bytes the renderer keeps for later renders, shared by every kind of data. When it is full,
   * the entries used least recently go first, whatever their kind. `0` keeps nothing but the fonts
   * from `registerFont`. Sizes are estimates, and the process uses memory this does not count.
   * @default 128 MiB
   */
  maxBytes?: number;
  /**
   * Whether to keep decoded images, and the SVG rasters and downscaled bitmaps drawn from them.
   * @default "auto"
   */
  images?: CacheMode;
  /**
   * Whether to keep resolved glyphs, and the masks rasterized from glyphs and shapes.
   * @default "auto", or `"none"` after `setGlyphCacheMaxBytes(0)` where the package exports it
   */
  glyphs?: CacheMode;
  /**
   * Whether to keep decoded font files, each counted once, once no render uses them. With `"auto"`,
   * the files renders used least recently are evicted one at a time when `maxBytes` is full, and
   * load again when a render lists them. `"none"` releases each file as the last render using it
   * finishes. A render keeps every file it uses until it finishes. Fonts from `registerFont` always
   * stay and count toward `maxBytes`.
   * @default "auto"
   */
  fonts?: CacheMode;
}

/**
 * What a renderer's caches hold and how their lookups went since the renderer was created, under
 * the keys `CacheOptions` uses. Sizes are the same estimates `maxBytes` counts, not process
 * memory.
 */
export interface CacheStats {
  /** The pool that every kind of data shares. */
  memory: CacheMemoryStats;
  /** Decoded images and the bitmaps drawn from them. */
  images: CacheKindStats;
  /** Resolved glyphs and their masks. */
  glyphs: CacheKindStats;
  /** Decoded font files, registered ones included. */
  fonts: CacheKindStats;
}

/** The bytes the pool holds, every kind of data together. */
export interface CacheMemoryStats {
  /** The bytes held now. */
  current: number;
  /** The most bytes held at once since the renderer was created. */
  high: number;
}

/** What one kind of data holds and how lookups for it went. */
export interface CacheKindStats {
  /** The bytes held now. For `fonts`, fonts from `registerFont` included. */
  current: number;
  /** Lookups that found what they looked for. */
  hits: number;
  /** Lookups that found nothing and did the work again. */
  misses: number;
  /** Entries released to stay within `maxBytes`. */
  evictions: number;
}

/** Options for constructing a renderer. */
export interface RendererOptions {
  /** The memory the renderer keeps across renders. */
  cache?: CacheOptions;
  /** @deprecated Use `cache.maxBytes`. */
  cacheMaxBytes?: number;
  /**
   * `0` keeps no fonts, as `cache.fonts: "none"` does. Any other value has no effect, as fonts
   * share `cache.maxBytes`.
   * @deprecated Use `cache.fonts`.
   */
  fontCacheMaxBytes?: number;
}

/** @deprecated Use `RendererOptions`. */
export type FontCacheOptions = RendererOptions;
