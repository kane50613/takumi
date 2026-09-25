import { COMMENT_NODE, DOCUMENT_NODE, ELEMENT_NODE, parse, renderSync, TEXT_NODE } from "ultrahtml";
import type {
  DocumentNode as UltraHtmlDocumentNode,
  ElementNode as UltraHtmlElementNode,
  Node as UltraHtmlNode,
} from "ultrahtml";
import { container, image, text } from "../helpers";
import type { ConvertedNodes } from "../root";
import type { Declarations, Node } from "../types";
import { NodeMetadataReader } from "../jsx/metadata";
import { isUnrenderedElement } from "../jsx/utils";
import { decodeHtmlEntities } from "./entities";

/**
 * Converts static HTML/SVG markup into Takumi nodes, keeping `<style>` contents
 * as stylesheets and turning `<img>` / `<svg>` into image nodes.
 */
export function fromStaticMarkup(markup: string): ConvertedNodes {
  return new NodeBuilder(new NodeMetadataReader()).build(parse(markup) as UltraHtmlDocumentNode);
}

class NodeBuilder {
  private readonly css: string[] = [];

  constructor(private readonly metadata: NodeMetadataReader) {}

  build(document: UltraHtmlDocumentNode): ConvertedNodes {
    return { nodes: this.nodes(document.children), css: this.css };
  }

  private nodes(children: UltraHtmlNode[]): Node[] {
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
        nodes.push(text({ text: value, preset: this.metadata.presets?.span }));
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
      this.nodes(element.children);
      return;
    }

    const metadata = this.elementMetadata(element);
    if (element.name === "br") {
      nodes.push(text({ text: "\n", preset: this.metadata.presets?.br, ...metadata }));
      return;
    }

    if (element.name === "img") {
      const src = element.attributes?.src;
      if (!src) {
        throw new Error("Image element must have a 'src' prop.");
      }

      nodes.push(image({ src: decodeHtmlEntities(src), ...dimensions(element), ...metadata }));
      return;
    }

    if (isUnrenderedElement(element.name)) {
      return;
    }

    if (element.name === "svg") {
      nodes.push(image({ src: renderSync(element), ...dimensions(element), ...metadata }));
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

    nodes.push(container({ children: this.nodes(element.children), ...metadata }));
  }

  private elementMetadata(element: UltraHtmlElementNode) {
    const props = element.attributes ? decodeAttributeMap(element.attributes) : {};

    return this.metadata.read(
      element.name,
      props,
      props.class,
      typeof props.style === "string" ? parseInlineStyle(props.style) : undefined,
    );
  }
}

/** Width and height attributes; values that are not finite numbers are dropped. */
function dimensions(element: UltraHtmlElementNode): { width?: number; height?: number } {
  return {
    width: parseDimension(element.attributes?.width),
    height: parseDimension(element.attributes?.height),
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
