import type { ContainerNode, Declarations, ImageNode, Node, NodeMetadata, TextNode } from "./types";

function applyStyle(node: Node, style?: Declarations) {
  if (style && Object.keys(style).length > 0) {
    node.style = style;
  }
}

function applyPreset(node: Node, preset?: Declarations) {
  if (preset && Object.keys(preset).length > 0) {
    node.preset = preset;
  }
}

function applyMetadata(node: Node, props: Partial<NodeMetadata>) {
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
}

export function container(props: Omit<ContainerNode, "type">): ContainerNode {
  const node: ContainerNode = {
    type: "container",
    children: props.children,
  };

  if (props.tw) {
    node.tw = props.tw;
  }

  applyMetadata(node, props);
  applyPreset(node, props.preset);
  applyStyle(node, props.style);

  return node;
}

export function text(text: string, style?: Declarations): TextNode;
export function text(props: Omit<TextNode, "type">): TextNode;

export function text(props: Omit<TextNode, "type"> | string, style?: Declarations): TextNode {
  if (typeof props === "string") {
    const node: TextNode = {
      type: "text",
      text: props,
    };

    applyStyle(node, style);

    return node;
  }

  const node: TextNode = {
    type: "text",
    text: props.text,
  };

  if (props.tw) {
    node.tw = props.tw;
  }

  applyMetadata(node, props);
  applyPreset(node, props.preset);
  applyStyle(node, style ?? props.style);

  return node;
}

export function image(props: Omit<ImageNode, "type">): ImageNode {
  const node: ImageNode = {
    type: "image",
    src: props.src,
    width: props.width,
    height: props.height,
  };

  if (props.tw) {
    node.tw = props.tw;
  }

  applyMetadata(node, props);
  applyPreset(node, props.preset);
  applyStyle(node, props.style);

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
