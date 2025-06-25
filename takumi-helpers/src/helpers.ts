import type { ContainerNode, ImageNode, StyleInput, TextNode } from "./types";

export function container(props: Omit<ContainerNode, "type">): ContainerNode {
  return {
    type: "container",
    ...props,
  };
}

export function text(text: string, style?: StyleInput): TextNode {
  return {
    ...style,
    type: "text",
    text,
  };
}

export function image(src: string, style?: StyleInput): ImageNode {
  return {
    ...style,
    type: "image",
    src,
  };
}

export function style(style: StyleInput) {
  return style;
}

/**
 * Convert a number to a percentage struct.
 * @param percentage - The percentage to convert (0.0 - 100.0).
 * @returns The percentage struct.
 */
export function percentage(percentage: number) {
  return {
    percentage,
  };
}

export function vw(vw: number) {
  return {
    vw,
  };
}

export function vh(vh: number) {
  return {
    vh,
  };
}

export function em(em: number) {
  return {
    em,
  };
}

export function rem(rem: number) {
  return {
    rem,
  };
}
