import type { Style } from "./bindings/Style";

export * from "./bindings/AlignItems";
export * from "./bindings/BoxShadow";
export * from "./bindings/BoxShadowInput";
export * from "./bindings/Color";
export * from "./bindings/ColorInput";
export * from "./bindings/FlexDirection";
export * from "./bindings/FontWeight";
export * from "./bindings/Gap";
export * from "./bindings/Gradient";
export * from "./bindings/JustifyContent";
export * from "./bindings/ObjectFit";
export * from "./bindings/Position";
export * from "./bindings/SidesValue";
export * from "./bindings/Style";
export * from "./bindings/TextAlign";
export * from "./bindings/ValuePercentageAuto";

type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

type AnyNode = {
  type: string;
  [key: string]: JsonValue;
};

export type StyleInput = Partial<Style>;

export type Node = ContainerNode | TextNode | ImageNode | AnyNode;

export type ContainerNode = StyleInput & {
  type: "container";
  children: Node[];
};

export type TextNode = StyleInput & {
  type: "text";
  text: string;
};

export type ImageNode = StyleInput & {
  type: "image";
  src: string;
};
