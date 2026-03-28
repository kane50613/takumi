import type { ComponentProps, CSSProperties, ReactElement, ReactNode } from "react";
import { container, image, percentage, text } from "../helpers";
import type { Node, NodeMetadata, ReactElementLike } from "../types";
import { fromStaticMarkup, type FromStaticMarkupOptions } from "./markup";
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
export * from "./markup";

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

export interface FromJsxResult {
  node: Node;
  stylesheets: string[];
}

interface FromJsxTraversalResult {
  nodes: Node[];
  stylesheets: string[];
}

type HtmlProps = {
  className?: string;
  class?: string;
  id?: string;
  style?: string | CSSProperties;
  [key: string]: unknown;
};

type ReactModuleLike = {
  default?: ReactModuleLike;
  Fragment?: unknown;
  createElement: (
    type: unknown,
    props: Record<string, unknown> | null,
    ...children: unknown[]
  ) => ReactNode;
};

type ReactDomServerModule = {
  default?: ReactDomServerModule;
  renderToStaticMarkup: (node: ReactNode) => string;
};

const INVALID_HOOK_PATTERNS = [
  /Invalid hook call\./,
  /Cannot read properties of null \(reading 'use[A-Z][A-Za-z]+'\)/,
  /null is not an object \(evaluating 'dispatcher\.use[A-Z][A-Za-z]+'\)/,
];

let reactDomServerPromise: Promise<ReactDomServerModule | null> | undefined;
let reactModulePromise: Promise<ReactModuleLike | null> | undefined;

export async function fromJsx(
  element: ReactNode | ReactElementLike,
  options?: FromJsxOptions,
): Promise<FromJsxResult> {
  const resolvedOptions = {
    presets: getPresets(options),
    tailwindClassesProperty: options?.tailwindClassesProperty ?? "tw",
  } satisfies ResolvedFromJsxOptions;
  const result = containsReactContextProvider(element)
    ? await renderWithReactDomServer(element, resolvedOptions)
    : await fromJsxInternal(element, resolvedOptions).catch(async (error) => {
        if (!shouldFallbackToReactDomServer(error)) {
          throw error;
        }

        return renderWithReactDomServer(element, resolvedOptions);
      });
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
    return { nodes: [], stylesheets: [] };
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

function extractAttributes(
  props: HtmlProps,
  tailwindClassesProperty: string,
): Record<string, string> | undefined {
  const collectedAttributes: Record<string, string> = {};

  for (const [attributeName, attributeValue] of Object.entries(props)) {
    if (
      attributeName === "children" ||
      attributeName === "className" ||
      attributeName === "class" ||
      attributeName === "id" ||
      attributeName === "style" ||
      attributeName === tailwindClassesProperty ||
      attributeName === "ref" ||
      attributeName === "key" ||
      attributeName === "dangerouslySetInnerHTML" ||
      attributeName === "suppressHydrationWarning"
    ) {
      continue;
    }

    if (attributeValue === undefined || attributeValue === null || attributeValue === false) {
      continue;
    }

    if (typeof attributeValue === "function" || typeof attributeValue === "symbol") {
      continue;
    }

    if (typeof attributeValue === "object") {
      continue;
    }

    collectedAttributes[attributeName] = attributeValue === true ? "" : String(attributeValue);
  }

  if (Object.keys(collectedAttributes).length === 0) {
    return;
  }

  return collectedAttributes;
}

function getPresets(options?: FromJsxOptions): typeof defaultStylePresets | undefined {
  if (options?.defaultStyles === false) return;

  return options?.defaultStyles ?? defaultStylePresets;
}

function containsReactContextProvider(element: ReactNode | ReactElementLike): boolean {
  if (element === undefined || element === null || element === false) {
    return false;
  }

  if (element instanceof Promise) {
    return false;
  }

  if (typeof element === "object" && Symbol.iterator in element) {
    for (const child of element as Iterable<ReactNode>) {
      if (containsReactContextProvider(child)) {
        return true;
      }
    }

    return false;
  }

  if (!isValidElement(element)) {
    return false;
  }

  if (
    typeof element.type === "object" &&
    element.type !== null &&
    "$$typeof" in element.type &&
    element.type.$$typeof === Symbol.for("react.context")
  ) {
    return true;
  }

  return containsReactContextProvider(getElementChildren(element));
}

function shouldFallbackToReactDomServer(error: unknown): boolean {
  if (!(error instanceof Error)) {
    return false;
  }

  return INVALID_HOOK_PATTERNS.some((pattern) => pattern.test(error.message));
}

async function renderFunctionComponent(
  component: (props: unknown) => ReactNode,
  props: unknown,
  options: ResolvedFromJsxOptions,
): Promise<FromJsxTraversalResult> {
  return fromJsxInternal(component(props), options);
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

async function getReactModule(): Promise<ReactModuleLike | null> {
  reactModulePromise ??= import("react")
    .then((module) => (module.default ?? module) as ReactModuleLike)
    .catch(() => null);

  return reactModulePromise;
}

async function getReactDomServerModule(): Promise<ReactDomServerModule | null> {
  reactDomServerPromise ??= import("react-dom/server")
    .then((module) => (module.default ?? module) as ReactDomServerModule)
    .catch(() => null);

  return reactDomServerPromise;
}

async function renderWithReactDomServer(
  element: ReactNode | ReactElementLike,
  options: ResolvedFromJsxOptions,
): Promise<FromJsxTraversalResult> {
  const [reactModule, reactDomServerModule] = await Promise.all([
    getReactModule(),
    getReactDomServerModule(),
  ]);

  if (!reactModule || !reactDomServerModule) {
    throw new Error("react-dom/server is required for JSX fallback rendering.");
  }

  const markup = reactDomServerModule.renderToStaticMarkup(
    reactModule.createElement(reactModule.Fragment ?? undefined, null, element),
  );

  return fromStaticMarkup(markup, {
    defaultStyles: options.presets ?? defaultStylePresets,
    tailwindClassesProperty: options.tailwindClassesProperty,
  } satisfies FromStaticMarkupOptions);
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
  let output = "";

  for (const child of children) {
    const chunk = collectStyleText(child);
    if (chunk === undefined) return;
    output += chunk;
  }

  return output;
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
): Promise<FromJsxTraversalResult> {
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

  if (typeof element.type !== "string" || isHtmlVoidElement(element)) {
    return { nodes: [], stylesheets: [] };
  }

  const metadata = extractNodeMetadata(element, options);

  if (isHtmlElement(element, "br")) {
    return {
      nodes: [
        text({
          text: "\n",
          preset: options.presets?.span,
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
    return Promise.resolve({ nodes: [], stylesheets: [] });
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
