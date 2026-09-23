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

    return {
      tagName,
      className,
      id: props.id,
      dir: props.dir as NodeMetadata["dir"],
      lang: props.lang,
      attributes: this.attributes(props),
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

  private attributes(props: HtmlProps): Record<string, string> | undefined {
    let attributes: Record<string, string> | undefined;

    for (const [name, value] of Object.entries(props)) {
      if (nonAttributeProps.has(name) || name === this.tailwindClassesProperty) continue;

      if (value === false || !attributeValueTypes.has(typeof value)) continue;

      attributes ??= {};
      attributes[name] = value === true ? "" : String(value);
    }

    return attributes;
  }
}
