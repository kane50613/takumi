import type { ComponentProps, ReactElement, ReactNode } from "react";
import { container, image, text } from "../helpers";
import { rootResult, type ConvertedNodes } from "../root";
import type { Node, NodeMetadata, RgbaImage, ReactElementLike } from "../types";
import { NodeMetadataReader, type HtmlProps } from "./metadata";
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

export interface FromJsxResult {
  node: Node;
  css: string[];
  /** @deprecated Use `css` instead. */
  stylesheets: string[];
}

export async function fromJsx(
  element: ReactNode | ReactElementLike,
  options?: FromJsxOptions,
): Promise<FromJsxResult> {
  const builder = new NodeBuilder(new NodeMetadataReader(options), new Map(), { current: 0 });

  return rootResult(await builder.build(element));
}

const REACT_CONTEXT_TYPE = Symbol.for("react.context");
const REACT_PROVIDER_TYPE = Symbol.for("react.provider");
const REACT_CONSUMER_TYPE = Symbol.for("react.consumer");

const MAX_CONCURRENT_ITERABLE_RESOLUTION = 8;

function emptyResult(): ConvertedNodes {
  return { nodes: [], css: [] };
}

function nodeResult(node: Node): ConvertedNodes {
  return { nodes: [node], css: [] };
}

/** Converts React elements into Takumi nodes with the context values and hook ids of one traversal. */
class NodeBuilder implements RenderEnv {
  constructor(
    private readonly metadata: NodeMetadataReader,
    readonly contexts: ReadonlyMap<unknown, unknown>,
    readonly ids: { current: number },
  ) {}

  async build(element: ReactNode | ReactElementLike): Promise<ConvertedNodes> {
    if (element === undefined || element === null || element === false) {
      return emptyResult();
    }

    // If element is a server component, wait for it to resolve first
    if (element instanceof Promise) return this.build(await element);

    // If element is an iterable, collect the children
    if (typeof element === "object" && Symbol.iterator in element) return this.iterable(element);

    if (isValidElement(element)) return this.element(element);

    return nodeResult(text({ text: String(element), preset: this.metadata.presets?.span }));
  }

  private async element(element: ReactElementLike): Promise<ConvertedNodes> {
    const contextResult = this.tryContext(element);
    if (contextResult !== undefined) return contextResult;

    if (isFunctionComponent(element.type)) {
      return this.functionComponent(element.type, element.props);
    }

    const wrapperResult = this.tryWrapper(element);
    if (wrapperResult !== undefined) return wrapperResult;

    // Handle React fragments <></>
    if (isReactFragment(element)) {
      return this.children(element);
    }

    if (isHtmlElement(element, "style")) {
      const css = collectStyleText(getElementChildren(element));
      return {
        nodes: [],
        css: css && css.length > 0 ? [css] : [],
      };
    }

    if (isHtmlElement(element, "head")) {
      const children = await this.children(element);
      return {
        nodes: [],
        css: children.css,
      };
    }

    if (typeof element.type !== "string" || isUnrenderedElement(element.type)) {
      return emptyResult();
    }

    const metadata = this.elementMetadata(element);

    if (isHtmlElement(element, "br")) {
      return nodeResult(text({ text: "\n", preset: this.metadata.presets?.br, ...metadata }));
    }

    if (isHtmlElement(element, "img")) {
      if (!element.props.src) {
        throw new Error("Image element must have a 'src' prop.");
      }

      return nodeResult(
        image({ src: element.props.src, ...dimensions(element.props), ...metadata }),
      );
    }

    if (isHtmlElement(element, "svg")) {
      return nodeResult(
        image({ src: serializeSvg(element), ...dimensions(element.props), ...metadata }),
      );
    }

    const textChildren = collectText(element);
    if (textChildren !== undefined) {
      return nodeResult(text({ text: textChildren, ...metadata }));
    }

    const children = await this.children(element);

    return {
      nodes: [container({ children: children.nodes, ...metadata })],
      css: children.css,
    };
  }

  /**
   * Handles context provider/consumer elements natively: providers push their
   * value onto the traversal's context map (read back by the dispatcher's
   * `useContext`), consumers call their render prop with the current value.
   * A `react.context` element type is a provider on React 19 and a legacy
   * consumer on 18; the render-prop children shape disambiguates.
   */
  private tryContext(element: ReactElementLike): Promise<ConvertedNodes> | undefined {
    const type = element.type;
    if (typeof type !== "object" || type === null) return;

    const tag = getProperty(type, "$$typeof");

    if (tag === REACT_PROVIDER_TYPE) {
      return this.childrenWithContext(element, getProperty(type, "_context"));
    }

    if (tag === REACT_CONSUMER_TYPE) {
      return this.consumer(element, getProperty(type, "_context") ?? type);
    }

    if (tag === REACT_CONTEXT_TYPE) {
      if (typeof getElementChildren(element) === "function") {
        return this.consumer(element, type);
      }

      return this.childrenWithContext(element, type);
    }
  }

  private tryWrapper(element: ReactElementLike): Promise<ConvertedNodes> | undefined {
    if (isReactForwardRef(element.type)) {
      const { render } = element.type;
      return this.functionComponent((props) => render(props, null), element.props);
    }

    if (isReactMemo(element.type)) {
      const innerType = element.type.type;

      if (isFunctionComponent(innerType)) {
        return this.functionComponent(innerType, element.props);
      }

      return this.element({ ...element, type: innerType });
    }
  }

  private childrenWithContext(
    element: ReactElementLike,
    context: unknown,
  ): Promise<ConvertedNodes> {
    const contexts = new Map(this.contexts);
    contexts.set(context, getProperty(element.props, "value"));

    return new NodeBuilder(this.metadata, contexts, this.ids).children(element);
  }

  private consumer(element: ReactElementLike, context: unknown): Promise<ConvertedNodes> {
    const children = getElementChildren(element);
    if (!isFunctionComponent(children)) return this.children(element);

    return this.build(children(readContext(this, context)));
  }

  private async functionComponent(
    component: (props: unknown) => ReactNode,
    props: unknown,
  ): Promise<ConvertedNodes> {
    return this.build(await callWithDispatcher(component, props, this));
  }

  private children(element: ReactElementLike): Promise<ConvertedNodes> {
    const children = getElementChildren(element);
    if (children === undefined) {
      return Promise.resolve(emptyResult());
    }

    return this.build(children);
  }

  private async iterable(iterable: Iterable<ReactNode>): Promise<ConvertedNodes> {
    const groupedResults: ConvertedNodes[] = [];
    const inFlight = new Set<Promise<void>>();
    let index = 0;

    for (const element of iterable) {
      const currentIndex = index++;
      const task = this.build(element)
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

  private elementMetadata(element: ReactElementLike): NodeMetadata {
    const props = element.props as HtmlProps;
    const style = props.style;

    return this.metadata.read(
      typeof element.type === "string" ? element.type : undefined,
      props,
      props.className ?? props.class,
      typeof style === "object" && style !== null && Object.keys(style).length > 0
        ? style
        : undefined,
    );
  }
}

/** Width and height props as numbers; non-numeric strings become `NaN`. */
function dimensions({ width, height }: { width?: number | string; height?: number | string }): {
  width?: number;
  height?: number;
} {
  return {
    width: width !== undefined ? Number(width) : undefined,
    height: height !== undefined ? Number(height) : undefined,
  };
}

function getElementChildren(element: ReactElementLike): ReactNode | undefined {
  return getProperty(element.props, "children") as ReactNode | undefined;
}

/** An element's children as one string, when they are all text; fragments are looked through. */
function collectText(element: ReactElementLike): string | undefined {
  const children = getElementChildren(element);

  if (typeof children === "string") return children;
  if (typeof children === "number") return String(children);

  if (typeof children === "object" && children !== null && Symbol.iterator in children) {
    return collectTextFromIterable(children as Iterable<ReactNode>);
  }

  if (isValidElement(children) && isReactFragment(children)) {
    return collectText(children);
  }
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

function collectStyleTextFromIterable(children: Iterable<ReactNode>): string | undefined {
  const chunks: string[] = [];

  for (const child of children) {
    const chunk = collectStyleText(child);
    if (chunk === undefined) return;
    chunks.push(chunk);
  }

  return chunks.join("");
}
