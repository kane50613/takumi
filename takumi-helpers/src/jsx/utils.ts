import type { ComponentProps, JSX, ReactElement, ReactNode } from "react";
export type { ReactElementLike } from "../types";
import type { ReactElementLike } from "../types";

const unrenderedElements = new Set(["head", "meta", "link", "style", "script"]);

export function isUnrenderedElement(type: string): boolean {
  return unrenderedElements.has(type);
}

/** Whether `text` holds nothing but HTML's ASCII white space. */
export function isAsciiWhitespace(text: string): boolean {
  return /^[\t\n\f\r ]*$/.test(text);
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

type ComponentInstance = { props: unknown; state?: unknown; render(): ReactNode };

type ClassComponent = {
  new (props: unknown): ComponentInstance;
  defaultProps?: Record<string, unknown>;
  getDerivedStateFromProps?: (props: unknown, state: unknown) => unknown;
};

/** Whether `value` is a class extending React's `Component` or `PureComponent`. */
function isClassComponent(value: unknown): value is ClassComponent {
  return (
    typeof value === "function" &&
    typeof value.prototype === "object" &&
    value.prototype !== null &&
    "isReactComponent" in value.prototype
  );
}

/**
 * The function that renders an element of `type`, or `undefined` when `type` is not a component.
 * A class component mounts as React mounts it: default props, the constructor, derived state, then
 * `render`.
 */
export function componentRenderer(type: unknown): ((props: unknown) => ReactNode) | undefined {
  if (isClassComponent(type)) {
    return (props) => {
      const resolved = withDefaultProps(props, type.defaultProps);
      const instance = new type(resolved);

      instance.props = resolved;
      if (type.getDerivedStateFromProps) {
        const derived = type.getDerivedStateFromProps(resolved, instance.state);

        if (derived !== null && derived !== undefined) {
          instance.state = { ...(instance.state as object), ...(derived as object) };
        }
      }

      return instance.render();
    };
  }

  return isFunctionComponent(type) ? type : undefined;
}

function withDefaultProps(props: unknown, defaults: Record<string, unknown> | undefined): unknown {
  if (!defaults) {
    return props;
  }

  const resolved: Record<string, unknown> = { ...(props as Record<string, unknown>) };

  for (const [name, value] of Object.entries(defaults)) {
    if (resolved[name] === undefined) {
      resolved[name] = value;
    }
  }

  return resolved;
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
  return (
    hasReactMarker(type, REACT_FORWARD_REF_TYPE) &&
    "render" in type &&
    typeof type.render === "function"
  );
}

export function isReactMemo(type: unknown): type is MemoComponent {
  return hasReactMarker(type, REACT_MEMO_TYPE) && "type" in type;
}

export function isReactFragment(element: ReactElementLike): boolean {
  return element.type === REACT_FRAGMENT_TYPE;
}
