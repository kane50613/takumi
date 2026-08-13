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

function escapeAttr(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function styleObjectToString(styleObj: Record<string, unknown>): string {
  const declarations: string[] = [];

  for (const key in styleObj) {
    if (!Object.hasOwn(styleObj, key)) {
      continue;
    }

    declarations.push(`${camelToKebab(key)}:${String(styleObj[key]).trim()}`);
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

  if (typeof value === "boolean") {
    // For SVG serialization we want boolean attributes to be explicit like
    // `focusable="true"` to match react-dom server output.
    return `${attrName}="${String(value)}"`;
  }

  if (key === "style" && typeof value === "object") {
    const styleString = styleObjectToString(value as Record<string, unknown>);
    if (styleString) return `style="${escapeAttr(styleString)}"`;
  }

  return `${attrName}="${escapeAttr(String(value))}"`;
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
    parts.push(String(node));
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
