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

export type ReactElementLike = {
  type: string | symbol | ((props: unknown) => ReactElementLike) | ReactElementLike;
  props: unknown;
  $$typeof?: symbol;
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

export type ImageNode = NodeMetadata & {
  type: "image";
  src: string | Uint8Array | ArrayBuffer;
  width?: number;
  height?: number;
};
