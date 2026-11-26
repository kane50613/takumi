import type { FromJsxOptions } from ".";
import type { Declarations, NodeMetadata } from "../types";
import { defaultStylePresets } from "./style-presets";

export type HtmlProps = {
  className?: string;
  class?: string;
  id?: string;
  style?: string | Declarations;
  dir?: string;
  lang?: string;
  [key: string]: unknown;
};

const nonAttributeProps = new Set([
  "children",
  "className",
  "class",
  "id",
  "style",
  "ref",
  "key",
  "dangerouslySetInnerHTML",
  "defaultChecked",
  "defaultValue",
  "innerHTML",
  "suppressContentEditableWarning",
  "suppressHydrationWarning",
]);

/** The props React DOM writes `true` and `false` for, as it does any `aria-*` or `data-*` prop. */
const booleanishProps = new Set([
  "contentEditable",
  "draggable",
  "spellCheck",
  "value",
  "autoReverse",
  "externalResourcesRequired",
  "focusable",
  "preserveAlpha",
]);

/** The attribute names React DOM's server renderer writes for props that spell them otherwise: its `aliases`, then the props it writes under a fixed name. */
export const attributeNames: Record<string, string> = {
  acceptCharset: "accept-charset",
  htmlFor: "for",
  httpEquiv: "http-equiv",
  crossOrigin: "crossorigin",
  accentHeight: "accent-height",
  alignmentBaseline: "alignment-baseline",
  arabicForm: "arabic-form",
  baselineShift: "baseline-shift",
  capHeight: "cap-height",
  clipPath: "clip-path",
  clipRule: "clip-rule",
  colorInterpolation: "color-interpolation",
  colorInterpolationFilters: "color-interpolation-filters",
  colorProfile: "color-profile",
  colorRendering: "color-rendering",
  dominantBaseline: "dominant-baseline",
  enableBackground: "enable-background",
  fillOpacity: "fill-opacity",
  fillRule: "fill-rule",
  floodColor: "flood-color",
  floodOpacity: "flood-opacity",
  fontFamily: "font-family",
  fontSize: "font-size",
  fontSizeAdjust: "font-size-adjust",
  fontStretch: "font-stretch",
  fontStyle: "font-style",
  fontVariant: "font-variant",
  fontWeight: "font-weight",
  glyphName: "glyph-name",
  glyphOrientationHorizontal: "glyph-orientation-horizontal",
  glyphOrientationVertical: "glyph-orientation-vertical",
  horizAdvX: "horiz-adv-x",
  horizOriginX: "horiz-origin-x",
  imageRendering: "image-rendering",
  letterSpacing: "letter-spacing",
  lightingColor: "lighting-color",
  markerEnd: "marker-end",
  markerMid: "marker-mid",
  markerStart: "marker-start",
  maskType: "mask-type",
  overlinePosition: "overline-position",
  overlineThickness: "overline-thickness",
  paintOrder: "paint-order",
  "panose-1": "panose-1",
  pointerEvents: "pointer-events",
  renderingIntent: "rendering-intent",
  shapeRendering: "shape-rendering",
  stopColor: "stop-color",
  stopOpacity: "stop-opacity",
  strikethroughPosition: "strikethrough-position",
  strikethroughThickness: "strikethrough-thickness",
  strokeDasharray: "stroke-dasharray",
  strokeDashoffset: "stroke-dashoffset",
  strokeLinecap: "stroke-linecap",
  strokeLinejoin: "stroke-linejoin",
  strokeMiterlimit: "stroke-miterlimit",
  strokeOpacity: "stroke-opacity",
  strokeWidth: "stroke-width",
  textAnchor: "text-anchor",
  textDecoration: "text-decoration",
  textRendering: "text-rendering",
  transformOrigin: "transform-origin",
  underlinePosition: "underline-position",
  underlineThickness: "underline-thickness",
  unicodeBidi: "unicode-bidi",
  unicodeRange: "unicode-range",
  unitsPerEm: "units-per-em",
  vAlphabetic: "v-alphabetic",
  vHanging: "v-hanging",
  vIdeographic: "v-ideographic",
  vMathematical: "v-mathematical",
  vectorEffect: "vector-effect",
  vertAdvY: "vert-adv-y",
  vertOriginX: "vert-origin-x",
  vertOriginY: "vert-origin-y",
  wordSpacing: "word-spacing",
  writingMode: "writing-mode",
  xmlnsXlink: "xmlns:xlink",
  xHeight: "x-height",
  autoFocus: "autofocus",
  xlinkActuate: "xlink:actuate",
  xlinkArcrole: "xlink:arcrole",
  xlinkHref: "xlink:href",
  xlinkRole: "xlink:role",
  xlinkShow: "xlink:show",
  xlinkTitle: "xlink:title",
  xlinkType: "xlink:type",
  xmlBase: "xml:base",
  xmlLang: "xml:lang",
  xmlSpace: "xml:space",
};

/** Reads node metadata from element props under one set of {@link FromJsxOptions}. */
export class NodeMetadataReader {
  readonly presets: typeof defaultStylePresets | undefined;
  readonly tailwindClassesProperty: string;

  constructor(options?: FromJsxOptions) {
    this.presets =
      options?.defaultStyles === false
        ? undefined
        : (options?.defaultStyles ?? defaultStylePresets);
    this.tailwindClassesProperty = options?.tailwindClassesProperty ?? "tw";
  }

  /** `className` and `style` arrive already read, since JSX and markup spell them differently. */
  read(
    tagName: string | undefined,
    props: HtmlProps,
    className: string | undefined,
    style: Declarations | undefined,
  ): NodeMetadata {
    const tw = props[this.tailwindClassesProperty];
    const dir = direction(props.dir);

    return {
      tagName,
      className,
      id: textValue(props.id),
      dir,
      lang: textValue(props.lang),
      attributes: this.attributes(tagName, props, dir !== undefined),
      tw: typeof tw === "string" ? tw : undefined,
      style,
      preset: this.preset(tagName),
    };
  }

  private preset(tagName: string | undefined): Declarations | undefined {
    return this.presets && tagName !== undefined && tagName in this.presets
      ? this.presets[tagName as keyof typeof this.presets]
      : undefined;
  }

  /** The attributes `props` set that no field of the node holds, as `LIFTED_ATTRIBUTES` in takumi-core lists them. */
  private attributes(
    tagName: string | undefined,
    props: HtmlProps,
    dirInField: boolean,
  ): Record<string, string> | undefined {
    let attributes: Record<string, string> | undefined;

    for (const [name, value] of Object.entries(formControlProps(tagName, props))) {
      if (nonAttributeProps.has(name) || name === this.tailwindClassesProperty) {
        continue;
      }

      if (
        name === "lang" ||
        (name === "dir" && dirInField) ||
        (tagName === "img" && name === "src")
      ) {
        continue;
      }

      const text = attributeText(name, value);

      if (text === undefined) {
        continue;
      }

      attributes ??= {};
      attributes[attributeNames[name] ?? name] = text;
    }

    return attributes;
  }
}

/** `props` as React DOM's server renderer writes a form control's: an `<input>` reads `checked` and `value` from `defaultChecked` and `defaultValue` when they are absent, and a `<select>` or `<textarea>` holds its value instead of writing it. */
function formControlProps(tagName: string | undefined, props: HtmlProps): HtmlProps {
  if (tagName === "input") {
    return {
      ...props,
      checked: props.checked ?? props.defaultChecked,
      value: props.value ?? props.defaultValue,
    };
  }

  if (tagName === "select" || tagName === "textarea") {
    return { ...props, value: undefined };
  }

  return props;
}

/** A `<textarea>`'s text: its `value`, else its `defaultValue`, as React DOM's server renderer writes it in place of the children. */
export function textareaValue(props: {
  value?: unknown;
  defaultValue?: unknown;
}): string | undefined {
  return textValue(props.value ?? props.defaultValue);
}

/** The text React DOM writes for a prop `name` of `value`, `undefined` when it writes none. */
function attributeText(name: string, value: unknown): string | undefined {
  if (typeof value !== "boolean") {
    return textValue(value);
  }

  if (booleanishProps.has(name) || name.startsWith("aria-") || name.startsWith("data-")) {
    return String(value);
  }

  return value ? "" : undefined;
}

/** The text of a string, number, or bigint prop. */
function textValue(value: unknown): string | undefined {
  return typeof value === "string" || typeof value === "number" || typeof value === "bigint"
    ? String(value)
    : undefined;
}

/** The `dir` field a `dir` attribute sets, which only the lowercase `ltr` and `rtl` do, so any other spelling stays an attribute. */
function direction(value: unknown): NodeMetadata["dir"] {
  return value === "ltr" || value === "rtl" ? value : undefined;
}
