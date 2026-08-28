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
  }
}

export type NodeAttributes = Record<string, string>;

/** A declaration block, with custom properties allowed. */
export type Declarations = CSSProperties & {
  [key: `--${string}`]: string | number;
};

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

/** One entry of the `css` render option: stylesheet text, or a rule object. */
export type CssInput = string | StyleRule | AnimationRule;

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
  style?: CSSProperties;
  preset?: CSSProperties;
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
