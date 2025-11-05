import type { Style } from "./bindings/Style";

export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

export type AnyNode = {
  type: string;
  [key: string]: JsonValue;
};

export type PartialStyle = Partial<Style>;

export type Node = ContainerNode | TextNode | ImageNode | AnyNode;

export type ContainerNode = {
  type: "container";
  style?: PartialStyle;
  children?: Node[];
  tw?: string;
};

export type TextNode = {
  type: "text";
  text: string;
  style?: PartialStyle;
  tw?: string;
};

export type ImageNode = {
  type: "image";
  src: string;
  width?: number;
  height?: number;
  style?: PartialStyle;
  tw?: string;
};
