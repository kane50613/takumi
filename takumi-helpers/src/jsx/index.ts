import type { ComponentProps, CSSProperties, ReactElement, ReactNode } from "react";
import { container, image, percentage, text } from "../helpers";
import type { Node, NodeMetadata, RgbaImage, ReactElementLike } from "../types";
import { extractAttributes, getPresets, type HtmlProps } from "./metadata";
export type { HtmlProps } from "./metadata";
import { callWithDispatcher, getProperty, readContext, type RenderEnv } from "./dispatcher";
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
} from "./utils";

export * from "./style-presets";

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}

export type BitmapProps = RgbaImage & Omit<ComponentProps<"img">, "src" | "width" | "height">;

type BitmapImgProps = Omit<BitmapProps, keyof RgbaImage> & { src: RgbaImage };

/** An `<img>` fed by raw RGBA pixels instead of an encoded file. */
export function Bitmap({
  width,
  height,
  data,
  premultiplied,
  ...props
}: BitmapProps): ReactElement<BitmapImgProps, "img"> {
  return {
    type: "img",
    props: { ...props, src: { width, height, data, premultiplied } },
    key: null,
  };
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

interface ResolvedFromJsxOptions extends RenderEnv {
  defaultStyles: typeof defaultStylePresets | false;
  presets?: typeof defaultStylePresets;
  tailwindClassesProperty: string;
}

export interface FromJsxResult {
  node: Node;
  stylesheets: string[];
}

interface FromJsxTraversalResult {
  nodes: Node[];
  stylesheets: string[];
}

function emptyTraversalResult(): FromJsxTraversalResult {
  return { nodes: [], stylesheets: [] };
}

export async function fromJsx(
  element: ReactNode | ReactElementLike,
  options?: FromJsxOptions,
): Promise<FromJsxResult> {
  const resolvedOptions = {
    defaultStyles: resolveDefaultStyles(options),
    presets: getPresets(options?.defaultStyles),
    tailwindClassesProperty: options?.tailwindClassesProperty ?? "tw",
    contexts: new Map<unknown, unknown>(),
    ids: { current: 0 },
  } satisfies ResolvedFromJsxOptions;
  const result = await fromJsxInternal(element, resolvedOptions);
  const nodes = result.nodes;

  let node: Node;
  if (nodes.length === 0) {
    node = container({});
  } else if (nodes.length === 1 && nodes[0] !== undefined) {
    node = nodes[0];
  } else {
    node = container({
      children: nodes,
      style: {
        width: percentage(100),
        height: percentage(100),
      },
    });
  }

  return {
    node,
    stylesheets: result.stylesheets,
  };
}

async function fromJsxInternal(
  element: ReactNode | ReactElementLike,
  options: ResolvedFromJsxOptions,
): Promise<FromJsxTraversalResult> {
  if (element === undefined || element === null || element === false) {
    return emptyTraversalResult();
  }

  // If element is a server component, wait for it to resolve first
  if (element instanceof Promise) return fromJsxInternal(await element, options);

  // If element is an iterable, collect the children
  if (typeof element === "object" && Symbol.iterator in element)
    return collectIterable(element, options);

  if (isValidElement(element)) {
    const result = await processReactElement(element, options);
    return result;
  }

  return {
    nodes: [
      text({
        text: String(element),
        preset: options.presets?.span,
      }),
    ],
    stylesheets: [],
  };
}

function resolveDefaultStyles(options?: FromJsxOptions): typeof defaultStylePresets | false {
  if (options && "defaultStyles" in options) {
    return options.defaultStyles ?? defaultStylePresets;
  }

  return defaultStylePresets;
}

const REACT_CONTEXT_TYPE = Symbol.for("react.context");
const REACT_PROVIDER_TYPE = Symbol.for("react.provider");
const REACT_CONSUMER_TYPE = Symbol.for("react.consumer");

/**
 * Handles context provider/consumer elements natively: providers push their
 * value onto the traversal's context map (read back by the dispatcher's
 * `useContext`), consumers call their render prop with the current value.
 * A `react.context` element type is a provider on React 19 and a legacy
 * consumer on 18; the render-prop children shape disambiguates.
 */
function tryHandleContextElement(
  element: ReactElementLike,
  options: ResolvedFromJsxOptions,
): Promise<FromJsxTraversalResult> | undefined {
  const type = element.type;
  if (typeof type !== "object" || type === null) return;

  const tag = getProperty(type, "$$typeof");

  if (tag === REACT_PROVIDER_TYPE) {
    return collectChildrenWithContext(element, getProperty(type, "_context"), options);
  }

  if (tag === REACT_CONSUMER_TYPE) {
    return renderConsumer(element, getProperty(type, "_context") ?? type, options);
  }

  if (tag === REACT_CONTEXT_TYPE) {
    if (typeof getElementChildren(element) === "function") {
      return renderConsumer(element, type, options);
    }

    return collectChildrenWithContext(element, type, options);
  }
}

function collectChildrenWithContext(
  element: ReactElementLike,
  context: unknown,
  options: ResolvedFromJsxOptions,
): Promise<FromJsxTraversalResult> {
  const contexts = new Map(options.contexts);
  contexts.set(context, getProperty(element.props, "value"));

  return collectChildren(element, { ...options, contexts });
}

function renderConsumer(
  element: ReactElementLike,
  context: unknown,
  options: ResolvedFromJsxOptions,
): Promise<FromJsxTraversalResult> {
  const children = getElementChildren(element);
  if (!isFunctionComponent(children)) return collectChildren(element, options);

  return fromJsxInternal(children(readContext(options, context)), options);
}

async function renderFunctionComponent(
  component: (props: unknown) => ReactNode,
  props: unknown,
  options: ResolvedFromJsxOptions,
): Promise<FromJsxTraversalResult> {
  return fromJsxInternal(await callWithDispatcher(component, props, options), options);
}

function tryHandleComponentWrapper(
  element: ReactElementLike,
  options: ResolvedFromJsxOptions,
): Promise<FromJsxTraversalResult> | undefined {
  if (typeof element.type !== "object" || element.type === null) return;

  if (isReactForwardRef(element.type) && "render" in element.type) {
    const forwardRefType = element.type as {
      render: (props: unknown, ref: unknown) => ReactNode;
    };
    return renderFunctionComponent(
      (props) => forwardRefType.render(props, null),
      element.props,
      options,
    );
  }

  if (isReactMemo(element.type) && "type" in element.type) {
    const memoType = element.type as { type: unknown };
    const innerType = memoType.type;

    if (isFunctionComponent(innerType)) {
      return renderFunctionComponent(innerType, element.props, options);
    }

    const cloned: ReactElementLike = {
      ...element,
      type: innerType as ReactElementLike["type"],
    } as ReactElementLike;

    return processReactElement(cloned, options);
  }
}

function getElementChildren(element: ReactElementLike): ReactNode | undefined {
  if (typeof element.props === "object" && element.props !== null && "children" in element.props) {
    return element.props.children as ReactNode;
  }
}

function tryCollectTextChildren(element: ReactElementLike): string | undefined {
  if (!isValidElement(element)) return;
  const children = getElementChildren(element);

  if (typeof children === "string") return children;
  if (typeof children === "number") return String(children);

  if (Array.isArray(children)) {
    return collectTextFromIterable(children);
  }

  if (typeof children === "object" && children !== null && Symbol.iterator in children) {
    return collectTextFromIterable(children as Iterable<ReactNode>);
  }

  if (isValidElement(children) && isReactFragment(children)) {
    return tryCollectTextChildren(children);
  }
}

function collectStyleTextFromIterable(children: Iterable<ReactNode>): string | undefined {
  const chunks: string[] = [];

  for (const child of children) {
    const chunk = collectStyleText(child);
    if (chunk === undefined) return;
    chunks.push(chunk);
  }

  return chunks.join("");
}

function collectStyleText(node: ReactNode | ReactElementLike): string | undefined {
  if (typeof node === "string") return node;
  if (typeof node === "number") return String(node);
  if (
    node === null ||
    node === undefined ||
    typeof node === "boolean" ||
    typeof node === "symbol"
  ) {
    return "";
  }

  if (typeof node === "object" && Symbol.iterator in node) {
    return collectStyleTextFromIterable(node as Iterable<ReactNode>);
  }

  if (!isValidElement(node)) return;

  if (isReactFragment(node)) {
    return collectStyleText(getElementChildren(node));
  }

  const children = getElementChildren(node);
  if (children === undefined) return "";

  if (typeof children === "object" && children !== null && Symbol.iterator in children) {
    return collectStyleTextFromIterable(children as Iterable<ReactNode>);
  }

  return collectStyleText(children);
}

function collectTextFromIterable(children: Iterable<ReactNode>): string | undefined {
  const chunks: string[] = [];
  let hasText = false;

  for (const child of children) {
    // If any child is a React element, this is not pure text
    if (isValidElement(child)) return;

    if (typeof child === "string") {
      hasText = true;
      chunks.push(child);
      continue;
    }

    if (typeof child === "number") {
      hasText = true;
      chunks.push(String(child));
      continue;
    }

    return;
  }

  if (!hasText) return;

  return chunks.join("");
}

async function processReactElement(
  element: ReactElementLike,
  options: ResolvedFromJsxOptions,
): Promise<FromJsxTraversalResult> {
  const contextResult = tryHandleContextElement(element, options);
  if (contextResult !== undefined) return contextResult;

  if (isFunctionComponent(element.type)) {
    return renderFunctionComponent(element.type, element.props, options);
  }

  const wrapperResult = tryHandleComponentWrapper(element, options);
  if (wrapperResult !== undefined) return wrapperResult;

  // Handle React fragments <></>
  if (isReactFragment(element)) {
    return collectChildren(element, options);
  }

  if (isHtmlElement(element, "style")) {
    const css = collectStyleText(getElementChildren(element));
    return {
      nodes: [],
      stylesheets: css && css.length > 0 ? [css] : [],
    };
  }

  if (isHtmlElement(element, "head")) {
    const children = await collectChildren(element, options);
    return {
      nodes: [],
      stylesheets: children.stylesheets,
    };
  }

  if (typeof element.type !== "string" || isHtmlVoidElement(element.type)) {
    return emptyTraversalResult();
  }

  const metadata = extractNodeMetadata(element, options);

  if (isHtmlElement(element, "br")) {
    return {
      nodes: [
        text({
          text: "\n",
          preset: options.presets?.br,
          ...metadata,
        }),
      ],
      stylesheets: [],
    };
  }

  if (isHtmlElement(element, "img")) {
    return {
      nodes: [createImageElement(element, options)],
      stylesheets: [],
    };
  }

  if (isHtmlElement(element, "svg")) {
    return {
      nodes: [createSvgElement(element, options)],
      stylesheets: [],
    };
  }

  const textChildren = tryCollectTextChildren(element);
  if (textChildren !== undefined) {
    return {
      nodes: [
        text({
          text: textChildren,
          ...metadata,
        }),
      ],
      stylesheets: [],
    };
  }

  const children = await collectChildren(element, options);

  return {
    nodes: [
      container({
        children: children.nodes,
        ...metadata,
      }),
    ],
    stylesheets: children.stylesheets,
  };
}

function createImageElement(
  element: ReactElement<ComponentProps<"img">, "img">,
  options: ResolvedFromJsxOptions,
) {
  if (!element.props.src) {
    throw new Error("Image element must have a 'src' prop.");
  }

  const metadata = extractNodeMetadata(element, options);

  const width = element.props.width !== undefined ? Number(element.props.width) : undefined;
  const height = element.props.height !== undefined ? Number(element.props.height) : undefined;

  return image({
    src: element.props.src,
    width,
    height,
    ...metadata,
  });
}

function createSvgElement(
  element: ReactElement<ComponentProps<"svg">, "svg">,
  options: ResolvedFromJsxOptions,
) {
  const metadata = extractNodeMetadata(element, options);
  const svg = serializeSvg(element);

  const width = element.props.width !== undefined ? Number(element.props.width) : undefined;
  const height = element.props.height !== undefined ? Number(element.props.height) : undefined;

  return image({
    src: svg,
    width,
    height,
    ...metadata,
  });
}

function extractStyle(
  element: ReactElementLike,
  options: ResolvedFromJsxOptions,
): { preset?: CSSProperties; style?: CSSProperties } {
  const presets = options.presets;
  const preset =
    presets && typeof element.type === "string" && element.type in presets
      ? presets[element.type as keyof typeof presets]
      : undefined;

  const inlineStyle =
    typeof element.props === "object" &&
    element.props !== null &&
    "style" in element.props &&
    typeof element.props.style === "object" &&
    element.props.style !== null
      ? element.props.style
      : undefined;

  if (!inlineStyle) {
    return { preset };
  }

  for (const key in inlineStyle) {
    if (Object.hasOwn(inlineStyle, key)) {
      return { preset, style: inlineStyle };
    }
  }

  return { preset };
}

function extractTw(element: ReactElementLike, options: ResolvedFromJsxOptions): string | undefined {
  const propName = options.tailwindClassesProperty;

  if (typeof element.props !== "object" || element.props === null || !(propName in element.props))
    return;

  const tw = element.props[propName as keyof typeof element.props];
  if (typeof tw !== "string") return;

  return tw;
}

function extractNodeMetadata(
  element: ReactElementLike,
  options: ResolvedFromJsxOptions,
): NodeMetadata {
  const htmlProps = element.props as HtmlProps;
  const { preset, style } = extractStyle(element, options);
  const tw = extractTw(element, options);
  const attributes = extractAttributes(htmlProps, options.tailwindClassesProperty);

  return {
    tagName: typeof element.type === "string" ? element.type : undefined,
    className: htmlProps.className ?? htmlProps.class,
    id: htmlProps.id,
    dir: htmlProps.dir as NodeMetadata["dir"],
    lang: htmlProps.lang,
    attributes,
    tw,
    style,
    preset,
  };
}

function collectChildren(
  element: ReactElementLike,
  options: ResolvedFromJsxOptions,
): Promise<FromJsxTraversalResult> {
  const children = getElementChildren(element);
  if (children === undefined) {
    return Promise.resolve(emptyTraversalResult());
  }

  return fromJsxInternal(children, options);
}

const MAX_CONCURRENT_ITERABLE_RESOLUTION = 8;

async function collectIterable(
  iterable: Iterable<ReactNode>,
  options: ResolvedFromJsxOptions,
): Promise<FromJsxTraversalResult> {
  const groupedResults: FromJsxTraversalResult[] = [];
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

  const flattenedNodes: Node[] = [];
  const flattenedStylesheets: string[] = [];
  for (const group of groupedResults) {
    if (!group) continue;
    flattenedNodes.push(...group.nodes);
    flattenedStylesheets.push(...group.stylesheets);
  }

  return {
    nodes: flattenedNodes,
    stylesheets: flattenedStylesheets,
  };
}
