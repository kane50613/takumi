import type { FailedImage } from "@takumi-rs/helpers";
import type { FailedFont } from "./renderer";

/** Something a render could not draw as its source asks, and went on without. */
export type RenderWarning =
  | {
      code: "missing-glyphs";
      message: string;
      /** The characters no font in their fallback chain draws, each once in document order. */
      characters: string[];
    }
  | {
      code: "unresolved-family";
      message: string;
      /** A family `font-family` or `fontFamilies` names that no registered or listed font provides. */
      family: string;
    }
  | {
      code: "font-load-failed";
      message: string;
      /** The URL of the `@font-face` file or listed font, or else the key the renderer keeps the font under. */
      source: string;
      cause: unknown;
    }
  | {
      code: "image-load-failed";
      message: string;
      /** The URL of the image, which `images.throwOnError: false` left out. */
      url: string;
      cause: unknown;
    };

/** What a render could not draw as its source asks. */
export type RenderReport = { warnings: RenderWarning[] };

/**
 * How a render hands over its warnings.
 *
 * @internal
 */
export type WarningOptions = {
  /** Receives each warning of a render. Without it, each distinct warning prints once through `console.warn`. */
  onWarning?: (warning: RenderWarning) => void;
  /** Rejects a render that raised any warning, with a `TakumiError` of code `ERR_TAKUMI_STRICT`. */
  strict?: boolean;
};

/** The fields each {@link TakumiErrorCode} adds to its error. */
type TakumiErrorFields = {
  ERR_TAKUMI_STRICT: { warnings: RenderWarning[] };
};

/** The code of a {@link TakumiError}. */
export type TakumiErrorCode = keyof TakumiErrorFields;

/** An error a render rejects with, whose `code` decides the fields it has. */
export type TakumiError<C extends TakumiErrorCode = TakumiErrorCode> = C extends TakumiErrorCode
  ? Error & { name: "TakumiError"; code: C } & TakumiErrorFields[C]
  : never;

/** A copy of this module loaded by another binding brands its errors the same way. */
const brand: unique symbol = Symbol.for("takumi.error");

class StrictError extends Error {
  override readonly name = "TakumiError";
  readonly code = "ERR_TAKUMI_STRICT";
  readonly [brand] = true;

  constructor(readonly warnings: [RenderWarning, ...RenderWarning[]]) {
    const more = warnings.length - 1;

    super(more > 0 ? `${warnings[0].message} (and ${more} more)` : warnings[0].message);
  }
}

/** Whether `error` is a {@link TakumiError}, of `code` when given, from any copy of takumi. */
export function isTakumiError<C extends TakumiErrorCode = TakumiErrorCode>(
  error: unknown,
  code?: C,
): error is TakumiError<C> {
  return (
    typeof error === "object" &&
    error !== null &&
    brand in error &&
    error[brand] === true &&
    "code" in error &&
    (code === undefined || error.code === code)
  );
}

const shownCharacters = 20;

/** The `missing-glyphs` warning for `characters`. */
function missingGlyphs(characters: string[]): RenderWarning {
  const shown = characters.slice(0, shownCharacters).join("");
  const more = characters.length - shownCharacters;
  const rest = more > 0 ? ` or ${more} more characters` : "";
  const them = characters.length > 1 ? "them" : "it";

  return {
    code: "missing-glyphs",
    message: `No font draws "${shown}"${rest}. Add a font that covers ${them} to \`fonts\`, or return one from \`onMissingGlyphs\`.`,
    characters,
  };
}

/** The message of `cause`, which can be any thrown value. */
function messageOf(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

/**
 * The warnings for what a render found no font for, and for the images and fonts that failed to load.
 *
 * @internal
 */
export function warningsOf(
  missingCharacters: string[],
  unresolvedFamilies: string[],
  failedImages: FailedImage[],
  failedFonts: FailedFont[],
): RenderWarning[] {
  return [
    ...(missingCharacters.length > 0 ? [missingGlyphs(missingCharacters)] : []),
    ...unresolvedFamilies.map((family): RenderWarning => ({
      code: "unresolved-family",
      message: `No font provides the family "${family}". Add a font named "${family}" to \`fonts\`, or correct the name.`,
      family,
    })),
    ...failedImages.map(({ src, error }): RenderWarning => ({
      code: "image-load-failed",
      message: `Failed to load the image ${src}: ${messageOf(error)}`,
      url: src,
      cause: error,
    })),
    ...fontWarnings(failedFonts),
  ];
}

/**
 * The warnings for the fonts that failed to load.
 *
 * @internal
 */
export function fontWarnings(failedFonts: FailedFont[]): RenderWarning[] {
  return failedFonts.map(({ source, error }) => ({
    code: "font-load-failed",
    message: `Failed to load the font ${source}: ${messageOf(error)}`,
    source,
    cause: error,
  }));
}

/** What a warning is about, which tells it apart from another of its code. */
function subjectOf(warning: RenderWarning): string {
  switch (warning.code) {
    case "missing-glyphs":
      return warning.characters.join("");
    case "unresolved-family":
      return warning.family;
    case "font-load-failed":
      return warning.source;
    case "image-load-failed":
      return warning.url;
  }
}

const printedLimit = 1000;

/** The warnings printed so far, by code and subject, the oldest first. */
const printed = new Set<string>();

/** Prints `warning` through `console.warn`, unless this process printed it already. */
function printOnce(warning: RenderWarning): void {
  const key = `${warning.code}\u0000${subjectOf(warning)}`;

  if (printed.has(key)) {
    return;
  }

  if (printed.size >= printedLimit) {
    printed.delete(printed.values().next().value ?? "");
  }

  printed.add(key);
  console.warn(`[takumi] ${warning.message}`);
}

/**
 * Hands each of `warnings` to `onWarning`, or prints it, then rejects under `strict` when there is any.
 *
 * @internal
 */
export function raiseWarnings(
  warnings: RenderWarning[],
  { onWarning = printOnce, strict = false }: WarningOptions,
): void {
  for (const warning of warnings) {
    onWarning(warning);
  }

  const [first, ...rest] = warnings;

  if (strict && first) {
    throw new StrictError([first, ...rest]);
  }
}
