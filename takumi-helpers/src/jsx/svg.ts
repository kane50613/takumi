import type { ComponentProps, ReactElement } from "react";
import {
  camelToKebab,
  isFunctionComponent,
  isReactForwardRef,
  isReactMemo,
  isValidElement,
  type ReactElementLike,
} from "./utils";

function isTextNode(node: unknown): node is string | number {
  return typeof node === "string" || typeof node === "number";
}

// https://www.w3.org/TR/xml/#NT-Name
const xmlNameStartChars =
  ":A-Z_a-z\\u00C0-\\u00D6\\u00D8-\\u00F6\\u00F8-\\u02FF\\u0370-\\u037D\\u037F-\\u1FFF\\u200C-\\u200D\\u2070-\\u218F\\u2C00-\\u2FEF\\u3001-\\uD7FF\\uF900-\\uFDCF\\uFDF0-\\uFFFD\\u{10000}-\\u{EFFFF}";
const xmlName = new RegExp(
  `^[${xmlNameStartChars}][${xmlNameStartChars}\\-.0-9\\u00B7\\u0300-\\u036F\\u203F-\\u2040]*$`,
  "u",
);
const cssPropertyName = /^(?:--|-?[A-Za-z_\u0080-\u{10FFFF}])[\w\u0080-\u{10FFFF}-]*$/u;
const cssClosingBrackets = new Map([
  ["(", ")"],
  ["[", "]"],
  ["{", "}"],
]);

function assertXmlName(name: string, kind: "element" | "attribute"): void {
  if (!xmlName.test(name)) {
    throw new Error(`Invalid SVG ${kind} name: ${JSON.stringify(name)}`);
  }
}

/** Whether `value` stays inside one CSS declaration; trailing semicolons are allowed. */
function isCssDeclarationValue(value: string): boolean {
  const expectedClosers: string[] = [];
  let quote: string | undefined;

  for (let index = 0; index < value.length; index++) {
    const char = value.charAt(index);

    if (char === "\\") {
      if (index === value.length - 1) return false;

      index++;
      continue;
    }

    if (quote) {
      if (char === quote) quote = undefined;
      continue;
    }

    if (char === '"' || char === "'") {
      quote = char;
      continue;
    }

    if (char === "/" && value.charAt(index + 1) === "*") {
      const commentEnd = value.indexOf("*/", index + 2);

      if (commentEnd === -1) return false;

      index = commentEnd + 1;
      continue;
    }

    const closer = cssClosingBrackets.get(char);

    if (closer) {
      expectedClosers.push(closer);
      continue;
    }

    if (char === ")" || char === "]" || char === "}") {
      if (expectedClosers.pop() !== char) return false;
      continue;
    }

    if (char === ";" && expectedClosers.length === 0) {
      return /^[;\s]*$/.test(value.slice(index));
    }
  }

  return quote === undefined && expectedClosers.length === 0;
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#x27;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function styleObjectToString(styleObj: Record<string, unknown>): string {
  const declarations: string[] = [];

  for (const key in styleObj) {
    if (!Object.hasOwn(styleObj, key)) {
      continue;
    }

    const property = camelToKebab(key);
    const value = String(styleObj[key]).trim();

    if (!cssPropertyName.test(property)) {
      throw new Error(`Invalid SVG style property: ${JSON.stringify(key)}`);
    }

    if (!isCssDeclarationValue(value)) {
      throw new Error(`Invalid SVG style value: ${JSON.stringify(value)}`);
    }

    declarations.push(`${property}:${value}`);
  }

  return declarations.join(";");
}

const propertiesToKebabCase = new Set([
  "stopColor",
  "stopOpacity",
  "strokeWidth",
  "strokeDasharray",
  "strokeDashoffset",
  "strokeLinecap",
  "strokeLinejoin",
  "fillRule",
  "clipRule",
  "colorInterpolationFilters",
  "floodColor",
  "floodOpacity",
  "accentHeight",
  "alignmentBaseline",
  "arabicForm",
  "baselineShift",
  "capHeight",
  "clipPath",
  "clipPathUnits",
  "colorInterpolation",
  "colorProfile",
  "colorRendering",
  "enableBackground",
  "fillOpacity",
  "fontFamily",
  "fontSize",
  "fontSizeAdjust",
  "fontStretch",
  "fontStyle",
  "fontVariant",
  "fontWeight",
  "glyphName",
  "glyphOrientationHorizontal",
  "glyphOrientationVertical",
  "horizAdvX",
  "horizOriginX",
  "imageRendering",
  "letterSpacing",
  "lightingColor",
  "markerEnd",
  "markerMid",
  "markerStart",
  "overlinePosition",
  "overlineThickness",
  "paintOrder",
  "preserveAspectRatio",
  "pointerEvents",
  "shapeRendering",
  "strokeMiterlimit",
  "strokeOpacity",
  "textAnchor",
  "textDecoration",
  "textRendering",
  "transformOrigin",
  "underlinePosition",
  "underlineThickness",
  "unicodeBidi",
  "unicodeRange",
  "unitsPerEm",
  "vectorEffect",
  "vertAdvY",
  "vertOriginX",
  "vertOriginY",
  "vAlphabetic",
  "vHanging",
  "vIdeographic",
  "vMathematical",
  "wordSpacing",
  "writingMode",
]);

function serializePropToAttrString(key: string, value: unknown): string | undefined {
  if (key === "children" || value == null) return;

  // Determine the final attribute name (handle className and known SVG mappings)
  let attrName: string;
  if (key === "className") {
    attrName = "class";
  } else if (propertiesToKebabCase.has(key)) {
    attrName = camelToKebab(key);
  } else {
    attrName = key;
  }

  assertXmlName(attrName, "attribute");

  if (typeof value === "boolean") {
    // For SVG serialization we want boolean attributes to be explicit like
    // `focusable="true"` to match react-dom server output.
    return `${attrName}="${String(value)}"`;
  }

  if (key === "style" && typeof value === "object") {
    const styleString = styleObjectToString(value as Record<string, unknown>);
    if (styleString) return `style="${escapeXml(styleString)}"`;
  }

  return `${attrName}="${escapeXml(String(value))}"`;
}

function pushSerializedAttributes(
  props: Record<string, unknown>,
  parts: string[],
  needsXmlns: boolean,
): void {
  let injectedXmlns = false;

  for (const key in props) {
    if (!Object.hasOwn(props, key)) {
      continue;
    }

    const attr = serializePropToAttrString(key, props[key]);
    if (attr === undefined) {
      continue;
    }

    parts.push(" ", attr);
    if (key === "xmlns") {
      injectedXmlns = true;
    }
  }

  if (needsXmlns && !injectedXmlns) {
    parts.push(' xmlns="http://www.w3.org/2000/svg"');
  }
}

const serializeElementNode = (
  obj: ReactElementLike,
  parts: string[],
  injectSvgXmlns: boolean,
): void => {
  const props = (obj.props as Record<string, unknown>) || {};

  if (isFunctionComponent(obj.type)) {
    serializeNode(obj.type(obj.props), parts, false);
    return;
  }

  // Fragments and other symbol types have no tag of their own; emit their children in place.
  if (typeof obj.type === "symbol") {
    serializeNode(props.children, parts, false);
    return;
  }

  if (isReactForwardRef(obj.type)) {
    serializeNode(obj.type.render(obj.props, null), parts, false);
    return;
  }

  if (isReactMemo(obj.type)) {
    serializeElementNode({ ...obj, type: obj.type.type }, parts, injectSvgXmlns);
    return;
  }

  // Only string types can be used as HTML/SVG tag names
  if (typeof obj.type !== "string") return;

  assertXmlName(obj.type, "element");
  parts.push("<", obj.type);
  pushSerializedAttributes(props, parts, injectSvgXmlns && obj.type === "svg");

  const children = props.children;
  parts.push(">");
  serializeNode(children, parts, false);
  parts.push("</", obj.type, ">");
};

function serializeNode(node: unknown, parts: string[], injectSvgXmlns: boolean): void {
  if (node === null || node === undefined || node === false) return;

  if (isTextNode(node)) {
    parts.push(escapeXml(String(node)));
    return;
  }

  if (Array.isArray(node)) {
    for (const child of node) {
      serializeNode(child, parts, false);
    }
    return;
  }

  if (!isValidElement(node)) return;

  serializeElementNode(node, parts, injectSvgXmlns);
}

export function serializeSvg(element: ReactElement<ComponentProps<"svg">, "svg">): string {
  const parts: string[] = [];
  serializeNode(element, parts, true);
  return parts.join("");
}
