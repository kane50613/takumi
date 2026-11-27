import type { ComponentProps, ReactElement } from "react";
import { attributeNames } from "./metadata";
import {
  camelToKebab,
  componentRenderer,
  isReactForwardRef,
  isReactMemo,
  isValidElement,
  type ReactElementLike,
} from "./utils";

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

// React DOM's `unitlessNumbers`, the style properties whose numbers it writes without `px`.
const unitlessNumbers = new Set(
  "animationIterationCount aspectRatio borderImageOutset borderImageSlice borderImageWidth boxFlex boxFlexGroup boxOrdinalGroup columnCount columns flex flexGrow flexPositive flexShrink flexNegative flexOrder gridArea gridRow gridRowEnd gridRowSpan gridRowStart gridColumn gridColumnEnd gridColumnSpan gridColumnStart fontWeight lineClamp lineHeight opacity order orphans scale tabSize widows zIndex zoom fillOpacity floodOpacity stopOpacity strokeDasharray strokeDashoffset strokeMiterlimit strokeOpacity strokeWidth MozAnimationIterationCount MozBoxFlex MozBoxFlexGroup MozLineClamp msAnimationIterationCount msFlex msZoom msFlexGrow msFlexNegative msFlexOrder msFlexPositive msFlexShrink msGridColumn msGridColumnSpan msGridRow msGridRowSpan WebkitAnimationIterationCount WebkitBoxFlex WebKitBoxFlexGroup WebkitBoxOrdinalGroup WebkitColumnCount WebkitColumns WebkitFlex WebkitFlexGrow WebkitFlexPositive WebkitFlexShrink WebkitLineClamp".split(
    " ",
  ),
);

/** The prefixes of the attribute names a serialized subtree writes. */
type Prefixes = Set<string>;

export function serializeSvg(element: ReactElement<ComponentProps<"svg">, "svg">): string {
  const { xmlns, ...props } = element.props;
  const prefixes: Prefixes = new Set();
  const markup = serializeNode(
    xmlns != null
      ? element
      : { ...element, props: { ...props, xmlns: "http://www.w3.org/2000/svg" } },
    prefixes,
  );

  if (
    !prefixes.has("xlink") ||
    Object.entries(props).some(
      ([key, value]) => value != null && attributeName(key) === "xmlns:xlink",
    )
  ) {
    return markup;
  }

  return `<svg xmlns:xlink="http://www.w3.org/1999/xlink"${markup.slice("<svg".length)}`;
}

function serializeNode(node: unknown, prefixes: Prefixes): string {
  if (typeof node === "string" || typeof node === "number") return escapeXml(String(node));

  if (Array.isArray(node)) return node.map((child) => serializeNode(child, prefixes)).join("");

  return isValidElement(node) ? serializeElement(node, prefixes) : "";
}

function serializeElement(element: ReactElementLike, prefixes: Prefixes): string {
  const { type } = element;
  const props = (element.props as Record<string, unknown>) || {};

  const render = componentRenderer(type);

  if (render) return serializeNode(render(element.props), prefixes);

  // Fragments and other symbol types have no tag of their own; emit their children in place.
  if (typeof type === "symbol") return serializeNode(props.children, prefixes);

  if (isReactForwardRef(type)) return serializeNode(type.render(element.props, null), prefixes);

  if (isReactMemo(type)) return serializeElement({ ...element, type: type.type }, prefixes);

  if (typeof type !== "string") return "";

  assertXmlName(type, "element");

  return `<${type}${serializeAttributes(props, prefixes)}>${serializeNode(props.children, prefixes)}</${type}>`;
}

function serializeAttributes(props: Record<string, unknown>, prefixes: Prefixes): string {
  let attributes = "";

  for (const [key, value] of Object.entries(props)) {
    if (key === "children" || value == null) continue;

    const name = attributeName(key);

    assertXmlName(name, "attribute");

    const separator = name.indexOf(":");

    if (separator !== -1) {
      prefixes.add(name.slice(0, separator));
    }

    const text =
      key === "style" && typeof value === "object" ? serializeStyle(value) : String(value);

    attributes += ` ${name}="${escapeXml(text)}"`;
  }

  return attributes;
}

function attributeName(prop: string): string {
  if (prop === "className") return "class";

  return attributeNames[prop] ?? prop;
}

function serializeStyle(style: object): string {
  return Object.entries(style)
    .map(([key, value]) => {
      const property = camelToKebab(key);
      const text =
        typeof value === "number" &&
        value !== 0 &&
        !key.startsWith("--") &&
        !unitlessNumbers.has(key)
          ? `${value}px`
          : String(value).trim();

      if (!cssPropertyName.test(property)) {
        throw new Error(`Invalid SVG style property: ${JSON.stringify(key)}`);
      }

      if (!isCssDeclarationValue(text)) {
        throw new Error(`Invalid SVG style value: ${JSON.stringify(text)}`);
      }

      return `${property}:${text}`;
    })
    .join(";");
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

function assertXmlName(name: string, kind: "element" | "attribute"): void {
  if (!xmlName.test(name)) {
    throw new Error(`Invalid SVG ${kind} name: ${JSON.stringify(name)}`);
  }
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#x27;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}
