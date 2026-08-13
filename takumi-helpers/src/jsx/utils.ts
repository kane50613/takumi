import type { ComponentProps, JSX, ReactElement, ReactNode } from "react";
export type { ReactElementLike } from "../types";
import type { ReactElementLike } from "../types";

const voidElements = new Set(["head", "meta", "link", "style", "script"]);

export function isHtmlVoidElement(type: string) {
  return voidElements.has(type);
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

export function isFunctionComponent(value: unknown): value is (props: unknown) => ReactNode {
  return typeof value === "function";
}

const REACT_FORWARD_REF_TYPE = Symbol.for("react.forward_ref");
const REACT_MEMO_TYPE = Symbol.for("react.memo");
const REACT_FRAGMENT_TYPE = Symbol.for("react.fragment");

type ForwardRefComponent = { render: (props: unknown, ref: unknown) => ReactNode };
type MemoComponent = { type: ReactElementLike["type"] };

function hasReactMarker(type: unknown, marker: symbol): type is { $$typeof: symbol } {
  return (
    typeof type === "object" && type !== null && "$$typeof" in type && type.$$typeof === marker
  );
}

export function isReactForwardRef(type: unknown): type is ForwardRefComponent {
  return hasReactMarker(type, REACT_FORWARD_REF_TYPE) && "render" in type;
}

export function isReactMemo(type: unknown): type is MemoComponent {
  return hasReactMarker(type, REACT_MEMO_TYPE) && "type" in type;
}

export function isReactFragment(element: ReactElementLike): boolean {
  return element.type === REACT_FRAGMENT_TYPE;
}
