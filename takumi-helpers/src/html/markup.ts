import { COMMENT_NODE, DOCUMENT_NODE, ELEMENT_NODE, parse, renderSync, TEXT_NODE } from "ultrahtml";
import type {
  DocumentNode as UltraHtmlDocumentNode,
  ElementNode as UltraHtmlElementNode,
  Node as UltraHtmlNode,
} from "ultrahtml";
import { container, image, text } from "../helpers";
import type { Declarations, Node, NodeMetadata } from "../types";
import { extractAttributes, getPresets, presetFor } from "../jsx/metadata";
import type { FromJsxOptions } from "../jsx";
import type { defaultStylePresets } from "../jsx/style-presets";
import { isUnrenderedElement } from "../jsx/utils";
import { decodeHtmlEntities } from "./entities";

export interface FromStaticMarkupOptions extends FromJsxOptions {}

export interface FromStaticMarkupResult {
  nodes: Node[];
  css: string[];
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
  const builder = new StaticMarkupBuilder(
    getPresets(options?.defaultStyles),
    options?.tailwindClassesProperty ?? "tw",
  );
  const nodes = builder.build((parse(markup) as UltraHtmlDocumentNode).children);

  return { nodes, css: builder.css };
}

class StaticMarkupBuilder {
  readonly css: string[] = [];

  constructor(
    private readonly presets: typeof defaultStylePresets | undefined,
    private readonly tailwindClassesProperty: string,
  ) {}

  build(children: UltraHtmlNode[]): Node[] {
    const nodes: Node[] = [];

    for (const child of children) {
      this.append(child, nodes);
    }

    return nodes;
  }

  private append(node: UltraHtmlNode, nodes: Node[]): void {
    if (node.type === COMMENT_NODE) {
      return;
    }

    if (node.type === TEXT_NODE) {
      const value = decodeHtmlEntities(node.value ?? "");
      if (value) {
        nodes.push(text({ text: value, preset: this.presets?.span }));
      }
      return;
    }

    if (node.type === DOCUMENT_NODE) {
      for (const child of node.children) {
        this.append(child, nodes);
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
        this.css.push(content);
      }
      return;
    }

    if (element.name === "head") {
      this.build(element.children);
      return;
    }

    const metadata = this.metadata(element);
    if (element.name === "br") {
      nodes.push(text({ text: "\n", preset: this.presets?.br, ...metadata }));
      return;
    }

    if (element.name === "img") {
      const src = element.attributes?.src;
      if (!src) {
        throw new Error("Image element must have a 'src' prop.");
      }

      nodes.push(imageNode(decodeHtmlEntities(src), element, metadata));
      return;
    }

    if (isUnrenderedElement(element.name)) {
      return;
    }

    if (element.name === "svg") {
      nodes.push(imageNode(renderSync(element), element, metadata));
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
      nodes.push(text({ text: decodeHtmlEntities(textContent), ...metadata }));
      return;
    }

    nodes.push(container({ children: this.build(element.children), ...metadata }));
  }

  private metadata(element: UltraHtmlElementNode): NodeMetadata {
    const props = element.attributes ? decodeAttributeMap(element.attributes) : {};
    const tw = props[this.tailwindClassesProperty];

    return {
      tagName: element.name,
      className: props.class,
      id: props.id,
      dir: props.dir as NodeMetadata["dir"],
      lang: props.lang,
      attributes: extractAttributes(props, this.tailwindClassesProperty),
      tw: typeof tw === "string" ? tw : undefined,
      style: typeof props.style === "string" ? parseInlineStyle(props.style) : undefined,
      preset: presetFor(this.presets, element.name),
    };
  }
}

function imageNode(src: string, element: UltraHtmlElementNode, metadata: NodeMetadata): Node {
  return image({
    src,
    width: parseDimension(element.attributes?.width),
    height: parseDimension(element.attributes?.height),
    ...metadata,
  });
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

function parseInlineStyle(styleText: string): Declarations | undefined {
  const style: Record<string, string> = {};
  let start = 0;
  let colon = -1;
  let depth = 0;

  const commit = (end: number) => {
    if (colon < 0) {
      return;
    }

    const name = styleText.slice(start, colon).trim();
    const value = styleText.slice(colon + 1, end).trim();

    if (name && value) {
      style[cssPropertyToJsProperty(name)] = value;
    }
  };

  for (let index = 0; index < styleText.length; index += 1) {
    const character = styleText[index];

    if (character === "\\") {
      index += 1;
    } else if (character === '"' || character === "'") {
      index = skipQuoted(styleText, index);
    } else if (character === "/" && styleText[index + 1] === "*") {
      index = skipComment(styleText, index);
    } else if (character === "(") {
      depth += 1;
    } else if (character === ")") {
      depth = Math.max(0, depth - 1);
    } else if (depth > 0) {
      continue;
    } else if (character === ":" && colon < 0) {
      colon = index;
    } else if (character === ";") {
      commit(index);
      start = index + 1;
      colon = -1;
    }
  }

  commit(styleText.length);

  return Object.keys(style).length > 0 ? style : undefined;
}

function skipQuoted(styleText: string, start: number): number {
  const quote = styleText[start];

  for (let index = start + 1; index < styleText.length; index += 1) {
    if (styleText[index] === "\\") {
      index += 1;
    } else if (styleText[index] === quote) {
      return index;
    }
  }

  return styleText.length;
}

function skipComment(styleText: string, start: number): number {
  const end = styleText.indexOf("*/", start + 2);

  return end < 0 ? styleText.length : end + 1;
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
