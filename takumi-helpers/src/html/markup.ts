import { COMMENT_NODE, DOCUMENT_NODE, ELEMENT_NODE, parse, renderSync, TEXT_NODE } from "ultrahtml";
import type {
  DocumentNode as UltraHtmlDocumentNode,
  ElementNode as UltraHtmlElementNode,
  Node as UltraHtmlNode,
} from "ultrahtml";
import type { CSSProperties } from "react";
import { container, image, text } from "../helpers";
import type { Node, NodeMetadata } from "../types";
import { extractAttributes, getPresets } from "../jsx/metadata";
import type { FromJsxOptions } from "../jsx";
import type { defaultStylePresets } from "../jsx/style-presets";
import { isHtmlVoidElement } from "../jsx/utils";
import { decodeHtmlEntities } from "./entities";

export interface FromStaticMarkupOptions extends FromJsxOptions {}

export interface FromStaticMarkupResult {
  nodes: Node[];
  stylesheets: string[];
}

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
  const result: FromStaticMarkupResult = { nodes: [], stylesheets: [] };
  const presets = getPresets(options?.defaultStyles);
  const tailwindClassesProperty = options?.tailwindClassesProperty ?? "tw";

  for (const child of document.children) {
    buildStaticNodes(child, presets, tailwindClassesProperty, result.nodes, result.stylesheets);
  }

  return result;
}

function buildStaticNodes(
  node: UltraHtmlNode,
  presets: typeof defaultStylePresets | undefined,
  tailwindClassesProperty: string,
  nodes: Node[],
  stylesheets: string[],
): void {
  if (node.type === COMMENT_NODE) {
    return;
  }

  if (node.type === TEXT_NODE) {
    const value = decodeHtmlEntities(node.value ?? "");
    if (value) {
      nodes.push(
        text({
          text: value,
          preset: presets?.span,
        }),
      );
    }
    return;
  }

  if (node.type === DOCUMENT_NODE) {
    for (const child of node.children) {
      buildStaticNodes(child, presets, tailwindClassesProperty, nodes, stylesheets);
    }
    return;
  }

  if (node.type !== ELEMENT_NODE) {
    return;
  }

  const element = node as UltraHtmlElementNode;
  if (element.name === "style") {
    let content = "";

    for (const child of element.children) {
      if (child.type === TEXT_NODE && typeof child.value === "string") {
        content += child.value;
      }
    }

    if (content) {
      stylesheets.push(content);
    }
    return;
  }

  if (element.name === "head") {
    const discardedNodes: Node[] = [];

    for (const child of element.children) {
      buildStaticNodes(child, presets, tailwindClassesProperty, discardedNodes, stylesheets);
    }
    return;
  }

  const metadata = extractStaticNodeMetadata(element, presets, tailwindClassesProperty);
  if (element.name === "br") {
    nodes.push(
      text({
        text: "\n",
        preset: presets?.br,
        ...metadata,
      }),
    );
    return;
  }

  if (element.name === "img") {
    const src = element.attributes?.src;
    if (!src) {
      throw new Error("Image element must have a 'src' prop.");
    }

    nodes.push(
      image({
        src,
        width: parseDimension(element.attributes?.width),
        height: parseDimension(element.attributes?.height),
        ...metadata,
      }),
    );
    return;
  }

  if (isHtmlVoidElement(element.name)) {
    return;
  }

  if (element.name === "svg") {
    nodes.push(
      image({
        src: renderSync(element),
        width: parseDimension(element.attributes?.width),
        height: parseDimension(element.attributes?.height),
        ...metadata,
      }),
    );
    return;
  }

  let onlyTextChildren = true;
  let textContent = "";

  for (const child of element.children) {
    if (child.type === COMMENT_NODE) {
      continue;
    }

    if (child.type !== TEXT_NODE) {
      onlyTextChildren = false;
      break;
    }

    textContent += child.value ?? "";
  }

  if (onlyTextChildren && textContent) {
    nodes.push(
      text({
        text: decodeHtmlEntities(textContent),
        ...metadata,
      }),
    );
    return;
  }

  const childNodes: Node[] = [];
  for (const child of element.children) {
    buildStaticNodes(child, presets, tailwindClassesProperty, childNodes, stylesheets);
  }

  nodes.push(
    container({
      children: childNodes,
      ...metadata,
    }),
  );
}

function extractStaticNodeMetadata(
  node: UltraHtmlElementNode,
  presets: typeof defaultStylePresets | undefined,
  tailwindClassesProperty: string,
): NodeMetadata {
  const props = node.attributes ? decodeAttributeMap(node.attributes) : {};
  const style = typeof props.style === "string" ? parseInlineStyle(props.style) : undefined;
  const attributes = extractAttributes(props, tailwindClassesProperty);
  const tw =
    typeof props[tailwindClassesProperty] === "string" ? props[tailwindClassesProperty] : undefined;
  const preset =
    presets && node.name in presets ? presets[node.name as keyof typeof presets] : undefined;

  return {
    tagName: node.name,
    className: props.class,
    id: props.id,
    dir: props.dir as NodeMetadata["dir"],
    lang: props.lang,
    attributes,
    tw,
    style,
    preset,
  };
}

function decodeAttributeMap(attributes: Record<string, string>): Record<string, string> {
  const decodedAttributes: Record<string, string> = {};

  for (const name in attributes) {
    const value = attributes[name];
    if (value !== undefined) {
      decodedAttributes[name] = decodeHtmlEntities(value);
    }
  }

  return decodedAttributes;
}

/**
 * Split a `style` attribute on declaration boundaries.
 *
 * Only a top-level `;` separates declarations. The character is legal inside a
 * value: `url(data:image/png;base64,...)` is how markup embeds an image without
 * a fetch, and a quoted family name such as `font-family: "Foo; Bar"` may carry
 * one too, as may an escaped `\;` outside either. Splitting on every `;`
 * truncates the value at the first one and leaves the remainder as a
 * colon-less fragment that is then discarded.
 *
 * A backslash escapes the next character in every state, which is how CSS
 * treats it inside and outside a string alike.
 */
function splitStyleDeclarations(styleText: string): string[] {
  const declarations: string[] = [];
  let start = 0;
  let depth = 0;
  let quote: string | undefined;
  let escaped = false;

  for (let index = 0; index < styleText.length; index += 1) {
    const character = styleText[index];

    if (escaped) {
      escaped = false;
      continue;
    }

    if (character === "\\") {
      escaped = true;
      continue;
    }

    if (quote !== undefined) {
      if (character === quote) {
        quote = undefined;
      }
      continue;
    }

    if (character === '"' || character === "'") {
      quote = character;
    } else if (character === "(") {
      depth += 1;
    } else if (character === ")") {
      depth = Math.max(0, depth - 1);
    } else if (character === ";" && depth === 0) {
      declarations.push(styleText.slice(start, index));
      start = index + 1;
    }
  }

  declarations.push(styleText.slice(start));

  return declarations;
}

function parseInlineStyle(styleText: string): CSSProperties | undefined {
  const style: Record<string, string> = {};

  for (const declaration of splitStyleDeclarations(styleText)) {
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
  if (property.startsWith("--")) {
    return property;
  }

  return property.replace(/-([a-z])/g, (_, character: string) => character.toUpperCase());
}

function parseDimension(value: string | undefined): number | undefined {
  if (!value) {
    return;
  }

  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}
