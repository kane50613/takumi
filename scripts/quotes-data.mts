// Prints the `CLDR_QUOTES` table of `takumi-core/src/style/properties/quotes.rs` from an extracted
// `cldr-misc-full` package: `bun scripts/quotes-data.mts path/to/cldr-misc-full/main`.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const root = process.argv[2];

if (!root) {
  throw new Error("pass the main directory of an extracted cldr-misc-full package");
}

const BASIC = String.fromCodePoint(0x201c, 0x201d, 0x2018, 0x2019);
const all = new Map<string, string>();

for (const locale of readdirSync(root).sort()) {
  const path = join(root, locale, "delimiters.json");

  if (!existsSync(path)) {
    continue;
  }

  const delimiters = JSON.parse(readFileSync(path, "utf8")).main[locale].delimiters;
  const marks: string[] = [
    delimiters.quotationStart,
    delimiters.quotationEnd,
    delimiters.alternateQuotationStart,
    delimiters.alternateQuotationEnd,
  ];

  // Blink takes a locale only when each of its marks is one UTF-16 code unit.
  if (marks.every((mark) => mark.length === 1)) {
    all.set(locale.toLowerCase().replaceAll("-", "_"), marks.join(""));
  }
}

const lookup = (locale: string) => {
  const subtags = locale.split("_");

  while (subtags.length > 0) {
    const marks = all.get(subtags.join("_"));

    if (marks) {
      return marks;
    }

    subtags.pop();
  }

  return undefined;
};

// A locale whose marks match what its shorter tag finds adds nothing.
for (const [locale, marks] of all) {
  const parent = locale.includes("_")
    ? lookup(locale.slice(0, locale.lastIndexOf("_")))
    : undefined;

  if (marks !== (parent ?? BASIC)) {
    console.log(`  ("${locale}", [${[...marks].map((mark) => `"${mark}"`).join(", ")}]),`);
  }
}
