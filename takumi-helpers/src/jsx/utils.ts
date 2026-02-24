import type { ComponentProps, JSX, ReactElement, ReactNode } from "react";

export type ReactElementLike = {
  type:
    | string
    | symbol
    | ((props: unknown) => ReactElementLike)
    | ReactElementLike;
  props: unknown;
  $$typeof?: symbol;
};

const voidElements = new Set(["head", "meta", "link", "style", "script"]);

export function isHtmlVoidElement(element: ReactElementLike) {
  return voidElements.has(element.type as string);
}

export function isHtmlElement<T extends keyof JSX.IntrinsicElements>(
  element: ReactElementLike,
  type: T,
): element is ReactElement<ComponentProps<T>, T> {
  return element.type === type && "props" in element;
}

export function isIntrinsicElement(
  element: ReactElementLike,
): element is ReactElementLike & {
  type: string;
  props: ComponentProps<keyof JSX.IntrinsicElements>;
} {
  return typeof element.type === "string" && "props" in element;
}

export function camelToKebab(camel: string): string {
  return camel.replace(/([A-Z])/g, "-$1").toLowerCase();
}

export function isValidElement(object: unknown): object is ReactElementLike {
  return typeof object === "object" && object !== null && "type" in object;
}

export function isFunctionComponent(
  value: unknown,
): value is (props: unknown) => ReactNode {
  return typeof value === "function";
}

export const REACT_FORWARD_REF_TYPE = Symbol.for("react.forward_ref");
export const REACT_MEMO_TYPE = Symbol.for("react.memo");
export const REACT_FRAGMENT_TYPE = Symbol.for("react.fragment");

export function isReactForwardRef(element: ReactElementLike): boolean {
  return element.$$typeof === REACT_FORWARD_REF_TYPE;
}

export function isReactMemo(element: ReactElementLike): boolean {
  return element.$$typeof === REACT_MEMO_TYPE;
}

export function isReactFragment(element: ReactElementLike): boolean {
  return element.type === REACT_FRAGMENT_TYPE;
}

export function getElementChildren(
  element: ReactElementLike,
): ReactNode | undefined {
  if (
    typeof element.props === "object" &&
    element.props !== null &&
    "children" in element.props
  ) {
    return element.props.children as ReactNode;
  }
}

function collectText(node: ReactNode | ReactElementLike): string | undefined {
  if (typeof node === "string") return node;
  if (typeof node === "number") return String(node);
  if (node === null || node === undefined || node === false) return "";
  if (!isValidElement(node)) return;

  if (isReactFragment(node)) {
    return collectText(getElementChildren(node));
  }

  const children = getElementChildren(node);
  if (children === undefined) return "";

  if (
    typeof children === "object" &&
    children !== null &&
    Symbol.iterator in children
  ) {
    let text = "";

    for (const child of children as Iterable<ReactNode>) {
      const chunk = collectText(child);
      if (chunk === undefined) return;
      text += chunk;
    }

    return text;
  }

  return collectText(children);
}

export function resolveJsxComponentWrapper(
  element: ReactElementLike,
): ReactNode | ReactElementLike | undefined {
  if (isFunctionComponent(element.type)) {
    return element.type(element.props);
  }

  if (typeof element.type !== "object" || element.type === null) return;

  if (isReactForwardRef(element.type) && "render" in element.type) {
    const forwardRefType = element.type as {
      render: (props: unknown, ref: unknown) => ReactNode;
    };
    return forwardRefType.render(element.props, null);
  }

  if (isReactMemo(element.type) && "type" in element.type) {
    const memoType = element.type as { type: unknown };
    const innerType = memoType.type;

    if (isFunctionComponent(innerType)) {
      return innerType(element.props);
    }

    return {
      ...element,
      type: innerType as ReactElementLike["type"],
    };
  }
}

function walkStylesheets(
  node: ReactNode | ReactElementLike,
  chunks: string[],
): void {
  if (node === null || node === undefined || node === false) return;

  if (typeof node === "object" && Symbol.iterator in node) {
    for (const child of node as Iterable<ReactNode>) {
      walkStylesheets(child, chunks);
    }
    return;
  }

  if (!isValidElement(node)) return;

  const resolvedWrapper = resolveJsxComponentWrapper(node);
  if (resolvedWrapper !== undefined) {
    walkStylesheets(resolvedWrapper, chunks);
    return;
  }

  if (isHtmlElement(node, "style")) {
    const css = collectText(getElementChildren(node));
    if (css && css.length > 0) {
      chunks.push(css);
    }
    return;
  }

  walkStylesheets(getElementChildren(node), chunks);
}

export function extractStylesheets(
  element: ReactNode | ReactElementLike,
): string[] {
  const chunks: string[] = [];
  walkStylesheets(element, chunks);
  return chunks;
}
