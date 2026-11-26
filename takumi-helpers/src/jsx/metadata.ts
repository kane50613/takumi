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
  "suppressHydrationWarning",
]);

/** The props React DOM writes `true` and `false` for, as it does any `aria-*` or `data-*` prop. */
const booleanishProps = new Set(["contentEditable", "draggable", "spellCheck"]);

/** The attribute names React DOM writes for props that spell them otherwise. */
const attributeNames: Record<string, string> = {
  acceptCharset: "accept-charset",
  htmlFor: "for",
  httpEquiv: "http-equiv",
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

    for (const [name, value] of Object.entries(props)) {
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
