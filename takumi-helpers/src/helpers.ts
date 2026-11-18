import type { CSSProperties } from "react";
import type { ContainerNode, Declarations, ImageNode, Node, NodeMetadata, TextNode } from "./types";

function applyMetadata(node: Node, props: Partial<NodeMetadata>, style = props.style) {
  if (props.tw) {
    node.tw = props.tw;
  }

  if (props.tagName !== undefined) {
    node.tagName = props.tagName;
  }

  if (props.className !== undefined) {
    node.className = props.className;
  }

  if (props.id !== undefined) {
    node.id = props.id;
  }

  if (props.dir !== undefined) {
    node.dir = props.dir;
  }

  if (props.lang !== undefined) {
    node.lang = props.lang;
  }

  if (props.attributes !== undefined) {
    node.attributes = props.attributes;
  }

  if (hasDeclarations(props.preset)) {
    node.preset = props.preset;
  }

  if (hasDeclarations(style)) {
    node.style = style;
  }
}

function hasDeclarations(declarations: Declarations | undefined): declarations is Declarations {
  return declarations !== undefined && Object.keys(declarations).length > 0;
}

export function container(props: Omit<ContainerNode, "type">): ContainerNode {
  const node: ContainerNode = {
    type: "container",
    children: props.children,
  };

  applyMetadata(node, props);

  return node;
}

export function text(text: string, style?: Declarations): TextNode;
export function text(props: Omit<TextNode, "type">): TextNode;

export function text(props: Omit<TextNode, "type"> | string, style?: Declarations): TextNode {
  const textProps = typeof props === "string" ? { text: props } : props;
  const node: TextNode = {
    type: "text",
    text: textProps.text,
  };

  applyMetadata(node, textProps, style ?? textProps.style);

  return node;
}

export function image(props: Omit<ImageNode, "type">): ImageNode {
  const node: ImageNode = {
    type: "image",
    src: props.src,
    width: props.width,
    height: props.height,
  };

  applyMetadata(node, props);

  return node;
}

/** @deprecated Type the object as `CSSProperties` from React instead. Will be removed in v3. */
export function style(style: Declarations): CSSProperties {
  return style;
}

/** @deprecated Write the CSS string, such as `"50%"`, instead. Will be removed in v3. */
export function percentage(percentage: number): `${number}%` {
  return `${percentage}%` as const;
}

/** @deprecated Write the CSS string, such as `"100vw"`, instead. Will be removed in v3. */
export function vw(vw: number): `${number}vw` {
  return `${vw}vw` as const;
}

/** @deprecated Write the CSS string, such as `"100vh"`, instead. Will be removed in v3. */
export function vh(vh: number): `${number}vh` {
  return `${vh}vh` as const;
}

/** @deprecated Write the CSS string, such as `"1.5em"`, instead. Will be removed in v3. */
export function em(em: number): `${number}em` {
  return `${em}em` as const;
}

/** @deprecated Write the CSS string, such as `"1.5rem"`, instead. Will be removed in v3. */
export function rem(rem: number): `${number}rem` {
  return `${rem}rem` as const;
}

/** @deprecated Write the CSS string, such as `"1fr"`, instead. Will be removed in v3. */
export function fr(fr: number): `${number}fr` {
  return `${fr}fr` as const;
}

/** @deprecated Write the CSS string, such as `"rgb(0 0 0 / 0.5)"`, instead. Will be removed in v3. */
export function rgba(
  r: number,
  g: number,
  b: number,
  a = 1,
): `rgb(${number} ${number} ${number} / ${number})` {
  return `rgb(${r} ${g} ${b} / ${a})` as const;
}
