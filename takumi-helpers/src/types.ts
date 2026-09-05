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
  interface CSSProperties {
    textFit?: TextFit;
    [key: `--${string}`]: string | number | undefined;
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

/**
 * A JSX element from any React-shaped runtime; Preact vnodes fit too, so
 * `fromJsx` and the render inputs accept them without casts.
 */
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
