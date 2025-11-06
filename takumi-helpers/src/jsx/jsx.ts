import type {
  ComponentProps,
  CSSProperties,
  ReactElement,
  ReactNode,
} from "react";
import { container, image, percentage, text } from "../helpers";
import type { Node, PartialStyle } from "../types";
import { stylePresets } from "./style-presets";
import { serializeSvg } from "./svg";
import {
  isFunctionComponent,
  isHtmlElement,
  isHtmlVoidElement,
  isReactForwardRef,
  isReactFragment,
  isReactMemo,
  isValidElement,
  type ReactElementLike,
} from "./utils";

declare module "react" {
  // biome-ignore lint/correctness/noUnusedVariables: used for type inference
  interface DOMAttributes<T> {
    tw?: string;
  }
}

export async function fromJsx(
  element: ReactNode | ReactElementLike,
): Promise<Node> {
  const result = await fromJsxInternal(element);

  if (result.length === 0) {
    return container({});
  }

  if (result.length === 1 && result[0] !== undefined) {
    return result[0];
  }

  return container({
    children: result,
    style: {
      width: percentage(100),
      height: percentage(100),
    },
  });
}

async function fromJsxInternal(
  element: ReactNode | ReactElementLike,
): Promise<Node[]> {
  if (element === undefined || element === null || element === false) return [];

  // If element is a server component, wait for it to resolve first
  if (element instanceof Promise) return fromJsxInternal(await element);

  // If element is an iterable, collect the children
  if (typeof element === "object" && Symbol.iterator in element)
    return collectIterable(element);

  if (isValidElement(element)) {
    const result = await processReactElement(element);
    return Array.isArray(result) ? result : result ? [result] : [];
  }

  return [text(String(element), stylePresets.span)];
}

function tryHandleComponentWrapper(
  element: ReactElementLike,
): Promise<Node[]> | undefined {
  if (typeof element.type !== "object" || element.type === null)
    return undefined;

  // Handle forwardRef components
  if (isReactForwardRef(element.type) && "render" in element.type) {
    const forwardRefType = element.type as {
      render: (props: unknown, ref: unknown) => ReactNode;
    };
    return fromJsxInternal(forwardRefType.render(element.props, null));
  }

  // Handle memo components
  if (isReactMemo(element.type) && "type" in element.type) {
    const memoType = element.type as { type: unknown };
    const innerType = memoType.type;

    if (isFunctionComponent(innerType)) {
      return fromJsxInternal(innerType(element.props));
    }

    const cloned: ReactElementLike = {
      ...element,
      type: innerType as ReactElementLike["type"],
    } as ReactElementLike;

    return processReactElement(cloned);
  }
}

function tryCollectTextChildren(
  element: ReactElementLike,
): Promise<string | undefined> {
  if (!isValidElement(element)) return Promise.resolve(undefined);

  const children =
    typeof element.props === "object" &&
    element.props !== null &&
    "children" in element.props
      ? element.props.children
      : undefined;

  if (typeof children === "string") return Promise.resolve(children);
  if (typeof children === "number") return Promise.resolve(String(children));

  if (Array.isArray(children)) {
    return Promise.resolve(collectTextFromChildren(children));
  }

  if (
    typeof children === "object" &&
    children !== null &&
    Symbol.iterator in children
  ) {
    return Promise.resolve(
      collectTextFromChildren(
        Array.from(children as Iterable<ReactElementLike>) as ReactNode[],
      ),
    );
  }

  if (isValidElement(children) && isReactFragment(children)) {
    return tryCollectTextChildren(children);
  }

  return Promise.resolve(undefined);
}

// Collects pure text children to prevent unnecessary container nodes
function collectTextFromChildren(children: ReactNode[]): string | undefined {
  // If any child is a React element, this is not pure text
  if (children.some((child) => isValidElement(child))) return;

  // All children are strings/numbers, concatenate them
  return children
    .map((child) => {
      if (typeof child === "string") return child;
      if (typeof child === "number") return String(child);
      // This shouldn't happen since we checked for elements above
      return "";
    })
    .join("");
}

async function processReactElement(element: ReactElementLike): Promise<Node[]> {
  if (isFunctionComponent(element.type)) {
    return fromJsxInternal(element.type(element.props));
  }

  const wrapperResult = tryHandleComponentWrapper(element);
  if (wrapperResult !== undefined) return wrapperResult;

  // Handle React fragments <></>
  if (isReactFragment(element)) {
    const children = await collectChildren(element);
    return children || [];
  }

  if (isHtmlVoidElement(element)) {
    return [];
  }

  if (isHtmlElement(element, "br")) {
    return [text("\n", stylePresets.span)];
  }

  if (isHtmlElement(element, "img")) {
    return [createImageElement(element)];
  }

  if (isHtmlElement(element, "svg")) {
    return [createSvgElement(element)];
  }

  const style = extractStyle(element) as PartialStyle;
  const tw = extractTw(element);

  const textChildren = await tryCollectTextChildren(element);
  if (textChildren !== undefined)
    return [
      text({
        text: textChildren,
        style,
        tw,
      }),
    ];

  const children = await collectChildren(element);

  return [
    container({
      children,
      style,
      tw,
    }),
  ];
}

function createImageElement(
  element: ReactElement<ComponentProps<"img">, "img">,
) {
  if (!element.props.src) {
    throw new Error("Image element must have a 'src' prop.");
  }

  const style = extractStyle(element) as PartialStyle;
  const tw = extractTw(element);

  return image({
    src: element.props.src,
    style,
    tw,
  });
}

function createSvgElement(element: ReactElement<ComponentProps<"svg">, "svg">) {
  const style = extractStyle(element) as PartialStyle;
  const tw = extractTw(element);
  const svg = serializeSvg(element);

  return image({
    style,
    src: svg,
    tw,
  });
}

// Takumi support the following WebKit features without the `Webkit` prefix
const webkitPropertiesMapping = {
  WebkitTextStroke: "textStroke",
  WebkitTextStrokeWidth: "textStrokeWidth",
  WebkitTextStrokeColor: "textStrokeColor",
} satisfies Partial<Record<keyof CSSProperties, keyof PartialStyle>>;

function extractStyle(element: ReactElementLike): PartialStyle {
  const base = {};

  if (typeof element.type === "string" && element.type in stylePresets) {
    Object.assign(
      base,
      stylePresets[element.type as keyof typeof stylePresets],
    );
  }

  const style =
    typeof element.props === "object" &&
    element.props !== null &&
    "style" in element.props &&
    typeof element.props.style === "object" &&
    element.props.style !== null
      ? element.props.style
      : undefined;

  if (style && Object.keys(style).length > 0) {
    for (const [from, to] of Object.entries(webkitPropertiesMapping)) {
      if (from in style) {
        base[to as keyof typeof base] = style[from as keyof typeof style];
        delete style[from as keyof typeof style];
      }
    }

    Object.assign(base, style);
  }

  return base;
}

function extractTw(element: ReactElementLike): string | undefined {
  if (
    typeof element.props !== "object" ||
    element.props === null ||
    !("tw" in element.props)
  )
    return undefined;

  return element.props.tw as string;
}

function collectChildren(element: ReactElementLike): Promise<Node[]> {
  if (
    typeof element.props !== "object" ||
    element.props === null ||
    !("children" in element.props)
  )
    return Promise.resolve([]);

  return fromJsxInternal(element.props.children as ReactNode);
}

function collectIterable(iterable: Iterable<ReactNode>): Promise<Node[]> {
  return Promise.all(
    Array.from(iterable).map((element) => fromJsxInternal(element)),
  ).then((results) => results.flat());
}
