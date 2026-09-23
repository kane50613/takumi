import type { ComponentProps, ReactElement, ReactNode } from "react";
import { container, image, text } from "../helpers";
import { rootResult } from "../root";
import type { ImageNode, Node, NodeMetadata, RgbaImage, ReactElementLike } from "../types";
import { extractAttributes, getPresets, presetFor, type HtmlProps } from "./metadata";
export type { HtmlProps } from "./metadata";
import { callWithDispatcher, getProperty, readContext, type RenderEnv } from "./dispatcher";
import type { defaultStylePresets } from "./style-presets";
import { serializeSvg } from "./svg";
import {
  isFunctionComponent,
  isHtmlElement,
  isUnrenderedElement,
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
  presets?: typeof defaultStylePresets;
  tailwindClassesProperty: string;
}

export interface FromJsxResult {
  node: Node;
  css: string[];
  /** @deprecated Use `css` instead. */
  stylesheets: string[];
}

interface FromJsxTraversalResult {
  nodes: Node[];
  css: string[];
}

function emptyTraversalResult(): FromJsxTraversalResult {
  return { nodes: [], css: [] };
}

function nodeResult(node: Node): FromJsxTraversalResult {
  return { nodes: [node], css: [] };
}

export async function fromJsx(
  element: ReactNode | ReactElementLike,
  options?: FromJsxOptions,
): Promise<FromJsxResult> {
  const { nodes, css } = await fromJsxInternal(element, {
    presets: getPresets(options?.defaultStyles),
    tailwindClassesProperty: options?.tailwindClassesProperty ?? "tw",
    contexts: new Map<unknown, unknown>(),
    ids: { current: 0 },
  });

  return rootResult(nodes, css);
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

  if (isValidElement(element)) return processReactElement(element, options);

  return nodeResult(text({ text: String(element), preset: options.presets?.span }));
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
  if (isReactForwardRef(element.type)) {
    const { render } = element.type;
    return renderFunctionComponent((props) => render(props, null), element.props, options);
  }

  if (isReactMemo(element.type)) {
    const innerType = element.type.type;

    if (isFunctionComponent(innerType)) {
      return renderFunctionComponent(innerType, element.props, options);
    }

    return processReactElement({ ...element, type: innerType }, options);
  }
}

function getElementChildren(element: ReactElementLike): ReactNode | undefined {
  return getProperty(element.props, "children") as ReactNode | undefined;
}

function tryCollectTextChildren(element: ReactElementLike): string | undefined {
  const children = getElementChildren(element);

  if (typeof children === "string") return children;
  if (typeof children === "number") return String(children);

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

  return collectStyleText(getElementChildren(node));
}

/** Joins string and number children; `undefined` when empty or when any child is neither. */
function collectTextFromIterable(children: Iterable<ReactNode>): string | undefined {
  let text: string | undefined;

  for (const child of children) {
    if (typeof child !== "string" && typeof child !== "number") return;

    text = (text ?? "") + child;
  }

  return text;
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
      css: css && css.length > 0 ? [css] : [],
    };
  }

  if (isHtmlElement(element, "head")) {
    const children = await collectChildren(element, options);
    return {
      nodes: [],
      css: children.css,
    };
  }

  if (typeof element.type !== "string" || isUnrenderedElement(element.type)) {
    return emptyTraversalResult();
  }

  const metadata = extractNodeMetadata(element, options);

  if (isHtmlElement(element, "br")) {
    return nodeResult(text({ text: "\n", preset: options.presets?.br, ...metadata }));
  }

  if (isHtmlElement(element, "img")) {
    if (!element.props.src) {
      throw new Error("Image element must have a 'src' prop.");
    }

    return nodeResult(imageNode(element.props.src, element.props, metadata));
  }

  if (isHtmlElement(element, "svg")) {
    return nodeResult(imageNode(serializeSvg(element), element.props, metadata));
  }

  const textChildren = tryCollectTextChildren(element);
  if (textChildren !== undefined) {
    return nodeResult(text({ text: textChildren, ...metadata }));
  }

  const children = await collectChildren(element, options);

  return {
    nodes: [container({ children: children.nodes, ...metadata })],
    css: children.css,
  };
}

function imageNode(
  src: ImageNode["src"],
  { width, height }: { width?: number | string; height?: number | string },
  metadata: NodeMetadata,
): ImageNode {
  return image({
    src,
    width: width !== undefined ? Number(width) : undefined,
    height: height !== undefined ? Number(height) : undefined,
    ...metadata,
  });
}

function extractNodeMetadata(
  element: ReactElementLike,
  options: ResolvedFromJsxOptions,
): NodeMetadata {
  const htmlProps = element.props as HtmlProps;
  const tagName = typeof element.type === "string" ? element.type : undefined;
  const style = htmlProps.style;
  const tw = getProperty(element.props, options.tailwindClassesProperty);

  return {
    tagName,
    className: htmlProps.className ?? htmlProps.class,
    id: htmlProps.id,
    dir: htmlProps.dir as NodeMetadata["dir"],
    lang: htmlProps.lang,
    attributes: extractAttributes(htmlProps, options.tailwindClassesProperty),
    tw: typeof tw === "string" ? tw : undefined,
    style:
      typeof style === "object" && style !== null && Object.keys(style).length > 0
        ? style
        : undefined,
    preset: presetFor(options.presets, tagName),
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
    const currentIndex = index++;
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

  return {
    nodes: groupedResults.flatMap((group) => group.nodes),
    css: groupedResults.flatMap((group) => group.css),
  };
}
