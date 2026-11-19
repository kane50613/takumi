import type { FailedImage } from "@takumi-rs/helpers/renderer";

const shownCharacters = 20;

/** Prints `message` with `console.warn` in the form takumi's Rust warnings take. */
function warn(message: string): void {
  console.warn(`takumi: ${message}`);
}

/** The message of `cause`, which can be any thrown value. */
function messageOf(cause: unknown): string {
  if (cause instanceof Error) {
    return cause.message;
  }

  try {
    return String(cause);
  } catch {
    return "a value that has no text form";
  }
}

/**
 * Warns that the font `source` names, the URL of an `@font-face` file or of a font listed by URL and
 * otherwise the key the renderer keeps the font under, failed to load with `error`.
 *
 * @internal
 */
export function warnFontLoadFailed(source: string, error: unknown): void {
  warn(`Failed to load the font ${source}: ${messageOf(error)}`);
}

/**
 * Warns about what a render found no font for, and about the images that failed to load.
 *
 * @internal
 */
export function warnUndrawn(
  missingCharacters: string[],
  unresolvedFamilies: string[],
  failedImages: FailedImage[],
): void {
  if (missingCharacters.length > 0) {
    const shown = missingCharacters.slice(0, shownCharacters).join("");
    const more = missingCharacters.length - shownCharacters;
    const rest = more > 0 ? ` or ${more} more characters` : "";
    const them = missingCharacters.length > 1 ? "them" : "it";

    warn(
      `No font draws "${shown}"${rest}. Add a font that covers ${them} to \`fonts\`, or return one from \`loadFallbackFonts\`.`,
    );
  }

  for (const family of unresolvedFamilies) {
    warn(
      `No font provides the family "${family}". Add a font named "${family}" to \`fonts\`, or correct the name.`,
    );
  }

  for (const { src, error } of failedImages) {
    warn(`Failed to load the image ${src}: ${messageOf(error)}`);
  }
}
