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

export function style(style: Declarations) {
  return style;
}

export function percentage(percentage: number) {
  return `${percentage}%` as const;
}

export function vw(vw: number) {
  return `${vw}vw` as const;
}

export function vh(vh: number) {
  return `${vh}vh` as const;
}

export function em(em: number) {
  return `${em}em` as const;
}

export function rem(rem: number) {
  return `${rem}rem` as const;
}

export function fr(fr: number) {
  return `${fr}fr` as const;
}

export function rgba(r: number, g: number, b: number, a = 1) {
  return `rgb(${r} ${g} ${b} / ${a})` as const;
}
