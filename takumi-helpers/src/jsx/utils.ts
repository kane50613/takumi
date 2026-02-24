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
