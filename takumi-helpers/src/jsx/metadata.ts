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

const attributeValueTypes = new Set(["string", "number", "bigint", "boolean"]);

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
      if (nonAttributeProps.has(name) || name === this.tailwindClassesProperty) continue;

      if (
        name === "lang" ||
        (name === "dir" && dirInField) ||
        (tagName === "img" && name === "src")
      ) {
        continue;
      }

      if (value === false || !attributeValueTypes.has(typeof value)) continue;

      attributes ??= {};
      attributes[name] = value === true ? "" : String(value);
    }

    return attributes;
  }
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
