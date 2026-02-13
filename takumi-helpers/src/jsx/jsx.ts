import type {
  ComponentProps,
  CSSProperties,
  ReactElement,
  ReactNode,
} from "react";
import { container, image, percentage, text } from "../helpers";
import type { Node } from "../types";
import { defaultStylePresets } from "./style-presets";
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

export * from "./style-presets";

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}

export interface FromJsxOptions {
  /**
   * Override or disable the default Chromium style presets.
   *
   * If an object is provided, all the default style presets will be overridden.
   *
   * If `false` is provided explicitly, no default style presets will be used.
   */
  defaultStyles?: typeof defaultStylePresets | false;
  /**
   * The JSX prop name used to pass Tailwind classes.
   *
   * @default "tw"
   */
  tailwindClassesProperty?: string;
}

interface ResolvedFromJsxOptions {
  presets?: typeof defaultStylePresets;
  tailwindClassesProperty: string;
}

export async function fromJsx(
  element: ReactNode | ReactElementLike,
  options?: FromJsxOptions,
): Promise<Node> {
  const result = await fromJsxInternal(element, {
    presets: getPresets(options),
    tailwindClassesProperty: options?.tailwindClassesProperty ?? "tw",
  });

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
  options: ResolvedFromJsxOptions,
): Promise<Node[]> {
  if (element === undefined || element === null || element === false) return [];

  // If element is a server component, wait for it to resolve first
  if (element instanceof Promise)
    return fromJsxInternal(await element, options);

  // If element is an iterable, collect the children
  if (typeof element === "object" && Symbol.iterator in element)
    return collectIterable(element, options);

  if (isValidElement(element)) {
    const result = await processReactElement(element, options);
    return Array.isArray(result) ? result : result ? [result] : [];
  }

  return [
    text({
      text: String(element),
      preset: options.presets?.span,
    }),
  ];
}

function getPresets(
  options?: FromJsxOptions,
): typeof defaultStylePresets | undefined {
  if (options?.defaultStyles === false) return;

  return options?.defaultStyles ?? defaultStylePresets;
}

function tryHandleComponentWrapper(
  element: ReactElementLike,
  options: ResolvedFromJsxOptions,
): Promise<Node[]> | undefined {
  if (typeof element.type !== "object" || element.type === null) return;

  // Handle forwardRef components
  if (isReactForwardRef(element.type) && "render" in element.type) {
    const forwardRefType = element.type as {
      render: (props: unknown, ref: unknown) => ReactNode;
    };
    return fromJsxInternal(forwardRefType.render(element.props, null), options);
  }

  // Handle memo components
  if (isReactMemo(element.type) && "type" in element.type) {
    const memoType = element.type as { type: unknown };
    const innerType = memoType.type;

    if (isFunctionComponent(innerType)) {
      return fromJsxInternal(innerType(element.props), options);
    }

    const cloned: ReactElementLike = {
      ...element,
      type: innerType as ReactElementLike["type"],
    } as ReactElementLike;

    return processReactElement(cloned, options);
  }
}

function tryCollectTextChildren(element: ReactElementLike): string | undefined {
  if (!isValidElement(element)) return;

  const children =
    typeof element.props === "object" &&
    element.props !== null &&
    "children" in element.props
      ? element.props.children
      : undefined;

  if (typeof children === "string") return children;
  if (typeof children === "number") return String(children);

  if (Array.isArray(children)) {
    return collectTextFromIterable(children);
  }

  if (
    typeof children === "object" &&
    children !== null &&
    Symbol.iterator in children
  ) {
    return collectTextFromIterable(children as Iterable<ReactNode>);
  }

  if (isValidElement(children) && isReactFragment(children)) {
    return tryCollectTextChildren(children);
  }
}

function collectTextFromIterable(
  children: Iterable<ReactNode>,
): string | undefined {
  let output = "";

  for (const child of children) {
    // If any child is a React element, this is not pure text
    if (isValidElement(child)) return;

    if (typeof child === "string") {
      output += child;
      continue;
    }

    if (typeof child === "number") {
      output += String(child);
      continue;
    }

    return;
  }

  return output;
}

async function processReactElement(
  element: ReactElementLike,
  options: ResolvedFromJsxOptions,
): Promise<Node[]> {
  if (isFunctionComponent(element.type)) {
    return fromJsxInternal(element.type(element.props), options);
  }

  const wrapperResult = tryHandleComponentWrapper(element, options);
  if (wrapperResult !== undefined) return wrapperResult;

  // Handle React fragments <></>
  if (isReactFragment(element)) {
    const children = await collectChildren(element, options);
    return children || [];
  }

  if (isHtmlVoidElement(element)) {
    return [];
  }

  if (isHtmlElement(element, "br")) {
    return [text({ text: "\n", preset: options.presets?.span })];
  }

  if (isHtmlElement(element, "img")) {
    return [createImageElement(element, options)];
  }

  if (isHtmlElement(element, "svg")) {
    return [createSvgElement(element, options)];
  }

  const { preset, style } = extractStyle(element, options);
  const tw = extractTw(element, options);

  const textChildren = tryCollectTextChildren(element);
  if (textChildren !== undefined)
    return [
      text({
        text: textChildren,
        preset,
        style,
        tw,
      }),
    ];

  const children = await collectChildren(element, options);

  return [
    container({
      children,
      preset,
      style,
      tw,
    }),
  ];
}

function createImageElement(
  element: ReactElement<ComponentProps<"img">, "img">,
  options: ResolvedFromJsxOptions,
) {
  if (!element.props.src) {
    throw new Error("Image element must have a 'src' prop.");
  }

  const { preset, style } = extractStyle(element, options);
  const tw = extractTw(element, options);

  const width =
    element.props.width !== undefined ? Number(element.props.width) : undefined;
  const height =
    element.props.height !== undefined
      ? Number(element.props.height)
      : undefined;

  return image({
    src: element.props.src,
    width,
    height,
    preset,
    style,
    tw,
  });
}

function createSvgElement(
  element: ReactElement<ComponentProps<"svg">, "svg">,
  options: ResolvedFromJsxOptions,
) {
  const { preset, style } = extractStyle(element, options);
  const tw = extractTw(element, options);
  const svg = serializeSvg(element);

  const width =
    element.props.width !== undefined ? Number(element.props.width) : undefined;
  const height =
    element.props.height !== undefined
      ? Number(element.props.height)
      : undefined;

  return image({
    preset,
    width,
    height,
    style,
    src: svg,
    tw,
  });
}

function extractStyle(
  element: ReactElementLike,
  options: ResolvedFromJsxOptions,
): { preset?: CSSProperties; style?: CSSProperties } {
  let preset: CSSProperties | undefined;
  let style: CSSProperties | undefined;

  const presets = options.presets;
  if (presets && typeof element.type === "string" && element.type in presets) {
    preset = presets[element.type as keyof typeof presets];
  }

  const inlineStyle =
    typeof element.props === "object" &&
    element.props !== null &&
    "style" in element.props &&
    typeof element.props.style === "object" &&
    element.props.style !== null
      ? element.props.style
      : undefined;

  if (inlineStyle) {
    for (const key in inlineStyle) {
      if (!Object.hasOwn(inlineStyle, key)) continue;

      style = inlineStyle;
      break;
    }
  }

  return { preset, style };
}

function extractTw(
  element: ReactElementLike,
  options: ResolvedFromJsxOptions,
): string | undefined {
  const propName = options.tailwindClassesProperty;

  if (
    typeof element.props !== "object" ||
    element.props === null ||
    !(propName in element.props)
  )
    return;

  const tw = element.props[propName as keyof typeof element.props];
  if (typeof tw !== "string") return;

  return tw;
}

function collectChildren(
  element: ReactElementLike,
  options: ResolvedFromJsxOptions,
): Promise<Node[]> {
  if (
    typeof element.props !== "object" ||
    element.props === null ||
    !("children" in element.props)
  )
    return Promise.resolve([]);

  return fromJsxInternal(element.props.children as ReactNode, options);
}

const MAX_CONCURRENT_ITERABLE_RESOLUTION = 8;

async function collectIterable(
  iterable: Iterable<ReactNode>,
  options: ResolvedFromJsxOptions,
): Promise<Node[]> {
  const groupedResults: Node[][] = [];
  const inFlight = new Set<Promise<void>>();
  let index = 0;

  for (const element of iterable) {
    const currentIndex = index;
    index += 1;

    const task = fromJsxInternal(element, options)
      .then((nodes) => {
        groupedResults[currentIndex] = nodes;
      })
      .finally(() => inFlight.delete(task));

    inFlight.add(task);

    if (inFlight.size >= MAX_CONCURRENT_ITERABLE_RESOLUTION) {
      await Promise.race(inFlight);
    }
  }

  await Promise.all(inFlight);

  const flattened: Node[] = [];
  for (const group of groupedResults) {
    if (group) flattened.push(...group);
  }

  return flattened;
}
