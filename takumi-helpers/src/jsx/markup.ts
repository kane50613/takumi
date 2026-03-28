import { COMMENT_NODE, DOCUMENT_NODE, ELEMENT_NODE, parse, renderSync, TEXT_NODE } from "ultrahtml";
import type {
  DocumentNode as UltraHtmlDocumentNode,
  ElementNode as UltraHtmlElementNode,
  Node as UltraHtmlNode,
} from "ultrahtml";
import type { CSSProperties } from "react";
import { container, image, text } from "../helpers";
import type { Node, NodeMetadata, ReactElementLike } from "../types";
import type { FromJsxOptions } from "./jsx";
import { defaultStylePresets } from "./style-presets";
import { isHtmlVoidElement } from "./utils";

export interface FromStaticMarkupOptions extends FromJsxOptions {}

export interface FromStaticMarkupResult {
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

/**
 * Convert static HTML/SVG markup into Takumi nodes.
 *
 * This is primarily used as the React DOM Server fallback for `fromJsx()`,
 * but it is also exposed as a public API for callers that already have
 * serialized markup.
 *
 * The output preserves extracted stylesheet contents from `<style>` tags and
 * converts `<img>` / `<svg>` elements into Takumi image nodes.
 */
export function fromStaticMarkup(
  markup: string,
  options?: FromStaticMarkupOptions,
): FromStaticMarkupResult {
  const document = parse(markup) as UltraHtmlDocumentNode;
  const nodes: Node[] = [];
  const stylesheets: string[] = [];
  const presets = getPresets(options);
  const tailwindClassesProperty = options?.tailwindClassesProperty ?? "tw";

  for (const child of document.children) {
    const result = buildStaticNodes(child, presets, tailwindClassesProperty);
    nodes.push(...result.nodes);
    stylesheets.push(...result.stylesheets);
  }

  return { nodes, stylesheets };
}

function buildStaticNodes(
  node: UltraHtmlNode,
  presets: typeof defaultStylePresets | undefined,
  tailwindClassesProperty: string,
): FromStaticMarkupResult {
  if (node.type === COMMENT_NODE) {
    return { nodes: [], stylesheets: [] };
  }

  if (node.type === TEXT_NODE) {
    const value = node.value ?? "";
    return {
      nodes: value
        ? [
            text({
              text: value,
              preset: presets?.span,
            }),
          ]
        : [],
      stylesheets: [],
    };
  }

  if (node.type === DOCUMENT_NODE) {
    return node.children.reduce(
      (result: FromStaticMarkupResult, child: UltraHtmlNode) => {
        const next = buildStaticNodes(child, presets, tailwindClassesProperty);
        result.nodes.push(...next.nodes);
        result.stylesheets.push(...next.stylesheets);
        return result;
      },
      { nodes: [], stylesheets: [] } as FromStaticMarkupResult,
    );
  }

  if (node.type !== ELEMENT_NODE) {
    return { nodes: [], stylesheets: [] };
  }

  const element = node as UltraHtmlElementNode;
  if (element.name === "style") {
    const content = element.children
      .filter((child) => child.type === TEXT_NODE && typeof child.value === "string")
      .map((child) => child.value)
      .join("");

    return { nodes: [], stylesheets: content ? [content] : [] };
  }

  if (isHtmlVoidElement({ type: element.name, props: {} } as ReactElementLike)) {
    return { nodes: [], stylesheets: [] };
  }

  const metadata = extractStaticNodeMetadata(element, presets, tailwindClassesProperty);
  if (element.name === "br") {
    return {
      nodes: [
        text({
          text: "\n",
          preset: presets?.span,
          ...metadata,
        }),
      ],
      stylesheets: [],
    };
  }

  if (element.name === "img") {
    const src = element.attributes?.src;
    if (!src) {
      throw new Error("Image element must have a 'src' prop.");
    }

    return {
      nodes: [
        image({
          src,
          width: parseDimension(element.attributes?.width),
          height: parseDimension(element.attributes?.height),
          ...metadata,
        }),
      ],
      stylesheets: [],
    };
  }

  if (element.name === "svg") {
    return {
      nodes: [
        image({
          src: renderSync(element),
          width: parseDimension(element.attributes?.width),
          height: parseDimension(element.attributes?.height),
          ...metadata,
        }),
      ],
      stylesheets: [],
    };
  }

  const children = element.children.reduce(
    (result: FromStaticMarkupResult, child: UltraHtmlNode) => {
      const next = buildStaticNodes(child, presets, tailwindClassesProperty);
      result.nodes.push(...next.nodes);
      result.stylesheets.push(...next.stylesheets);
      return result;
    },
    { nodes: [], stylesheets: [] } as FromStaticMarkupResult,
  );

  const onlyTextChildren = element.children.every(
    (child) => child.type === TEXT_NODE || child.type === COMMENT_NODE,
  );

  return {
    nodes: [
      onlyTextChildren
        ? text({
            text: children.nodes.map((child) => (child.type === "text" ? child.text : "")).join(""),
            ...metadata,
          })
        : container({
            children: children.nodes,
            ...metadata,
          }),
    ],
    stylesheets: children.stylesheets,
  };
}

function extractStaticNodeMetadata(
  node: UltraHtmlElementNode,
  presets: typeof defaultStylePresets | undefined,
  tailwindClassesProperty: string,
): NodeMetadata {
  const props = node.attributes ?? {};
  const style = typeof props.style === "string" ? parseInlineStyle(props.style) : undefined;
  const attributes = extractAttributes(props, tailwindClassesProperty);
  const preset =
    presets && node.name in presets ? presets[node.name as keyof typeof presets] : undefined;

  return {
    tagName: node.name,
    className: props.class,
    id: props.id,
    attributes,
    style,
    preset,
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

  return Object.keys(collectedAttributes).length > 0 ? collectedAttributes : undefined;
}

function parseInlineStyle(styleText: string): CSSProperties | undefined {
  const style: Record<string, string> = {};

  for (const declaration of styleText.split(";")) {
    const [property, ...valueParts] = declaration.split(":");
    if (!property || valueParts.length === 0) {
      continue;
    }

    const name = property.trim();
    const value = valueParts.join(":").trim();
    if (!name || !value) {
      continue;
    }

    style[cssPropertyToJsProperty(name)] = value;
  }

  return Object.keys(style).length > 0 ? (style as CSSProperties) : undefined;
}

function cssPropertyToJsProperty(property: string): string {
  return property.replace(/-([a-z])/g, (_, character: string) => character.toUpperCase());
}

function parseDimension(value: string | undefined): number | undefined {
  if (!value) {
    return;
  }

  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function getPresets(options?: FromStaticMarkupOptions): typeof defaultStylePresets | undefined {
  if (options?.defaultStyles === false) {
    return;
  }

  return options?.defaultStyles ?? defaultStylePresets;
}
