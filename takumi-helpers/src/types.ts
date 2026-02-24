import type { CSSProperties } from "react";

export type BaseNode = {
  tagName?: string;
  className?: string;
  id?: string;
  tw?: string;
  style?: CSSProperties;
  preset?: CSSProperties;
};

export type AnyNode = BaseNode & {
  type: string;
  [key: string]: unknown;
};

/**
 * @deprecated Use {import("csstype").Properties} or {import("react").CSSProperties} instead
 */
export type PartialStyle = CSSProperties;

export type Node = ContainerNode | TextNode | ImageNode | AnyNode;

export type ContainerNode = BaseNode & {
  type: "container";
  children?: Node[];
};

export type TextNode = BaseNode & {
  type: "text";
  text: string;
};

export type ImageNode = BaseNode & {
  type: "image";
  src: string;
  width?: number;
  height?: number;
};
