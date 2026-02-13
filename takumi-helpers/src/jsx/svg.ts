import type { ComponentProps, ReactElement } from "react";
import {
  camelToKebab,
  isFunctionComponent,
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
  return Object.keys(styleObj)
    .map(
      (k) =>
        `${k.replace(/([A-Z])/g, "-$1").toLowerCase()}:${String(styleObj[k]).trim()}`,
    )
    .join(";");
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

function serializePropToAttrString(
  key: string,
  value: unknown,
): string | undefined {
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

function propsToAttrStrings(props: Record<string, unknown>): string[] {
  return Object.entries(props)
    .map(([key, value]) => serializePropToAttrString(key, value))
    .filter((attr): attr is string => attr !== undefined);
}

const serializeElementNode = (
  obj: ReactElementLike,
  serializeFn: (n: unknown) => string,
): string => {
  const props = (obj.props as Record<string, unknown>) || {};

  if (isFunctionComponent(obj.type)) return serialize(obj.type(obj.props));

  // Handle symbols (like React fragments) - they can't be serialized as HTML/SVG tags
  if (typeof obj.type === "symbol") return "";

  // Only string types can be used as HTML/SVG tag names
  if (typeof obj.type !== "string") return "";

  const attrs = propsToAttrStrings(props);
  const children = props.children;
  const childrenString = Array.isArray(children)
    ? children.map((c) => serializeFn(c)).join("")
    : serializeFn(children);

  return `<${obj.type}${attrs.length > 0 ? ` ${attrs.join(" ")}` : ""}>${childrenString}</${obj.type}>`;
};

const serialize = (node: unknown): string => {
  if (node === null || node === undefined || node === false) return "";
  if (isTextNode(node)) return String(node);
  if (Array.isArray(node)) return node.map(serialize).join("");
  if (!isValidElement(node)) return "";

  return serializeElementNode(node, serialize);
};

export function serializeSvg(
  element: ReactElement<ComponentProps<"svg">, "svg">,
): string {
  const props = (element.props as Record<string, unknown>) || {};

  if (!("xmlns" in props)) {
    const cloned: ReactElement<ComponentProps<"svg">, "svg"> = {
      ...element,
      props: {
        ...props,
        xmlns: "http://www.w3.org/2000/svg",
      },
    } as ReactElement<ComponentProps<"svg">, "svg">;

    return serialize(cloned) || "";
  }

  return serialize(element) || "";
}
