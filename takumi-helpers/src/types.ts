import type { CSSProperties } from "react";

export type NodeAttributes = Record<string, string>;

export type NodeMetadata = {
  tagName?: string;
  className?: string;
  id?: string;
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
  src: string;
  width?: number;
  height?: number;
};
