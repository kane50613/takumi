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

/** The byte budget of each kind of data a renderer keeps across renders, where `0` keeps none of
 * that kind, except as `fonts` says. */
export interface CacheOptions {
  /**
   * Decoded images, and the SVG rasters and downscaled bitmaps drawn from them.
   * @default 64 MiB
   */
  images?: number;
  /**
   * SVG images laid out to drawing ops, which rasters and PDF and SVG output draw from.
   * @default 16 MiB
   */
  pictures?: number;
  /**
   * Parsed stylesheets.
   * @default 4 MiB
   */
  stylesheets?: number;
  /**
   * The rules `tw` classes compile to.
   * @default 1 MiB
   */
  tailwind?: number;
  /**
   * Resolved glyphs, and the masks rasterized from glyphs and shapes. Raise it for CJK text,
   * whose glyphs outgrow the default.
   * @default 8 MiB, or the budget the last `setGlyphCacheMaxBytes` call set where the package
   * exports it
   */
  glyphs?: number;
  /**
   * Where glyph outlines cross a decoration line, for `text-decoration-skip-ink`.
   * @default 1 MiB
   */
  intercepts?: number;
  /**
   * The faces `@font-face` rules add, weighed by the font files they hold.
   * @default 16 MiB
   */
  fontFaces?: number;
  /**
   * Decoded fonts that renders' `fonts` lists may keep registered. Past it, the least recently
   * used families are released and load again when a render lists them. Fonts from `registerFont`
   * stay, and so does the font loaded last, even past the budget alone. A running render keeps the
   * fonts it started with.
   * @default unbounded
   */
  fonts?: number;
}

/** Options for constructing a renderer. */
export interface RendererOptions {
  /** The byte budget of each kind of data the renderer keeps across renders. */
  cache?: CacheOptions;
  /** @deprecated Use `cache.images`. */
  cacheMaxBytes?: number;
  /** @deprecated Use `cache.fonts`. */
  fontCacheMaxBytes?: number;
}
