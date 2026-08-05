import { mkdir } from "node:fs/promises";
import { join } from "node:path";

const CSS_URL = "https://fonts.googleapis.com/css2?family=Inter:wght@400;700&display=swap";
const DIR = join(import.meta.dir, "fonts");

/** Downloads Inter 400/700 as single-file ttf (curl UA skips subsets) and caches them. */
export async function interFonts(): Promise<{ regular: string; bold: string }> {
  const regular = join(DIR, "inter-400.ttf");
  const bold = join(DIR, "inter-700.ttf");

  if ((await Bun.file(regular).exists()) && (await Bun.file(bold).exists())) {
    return { regular, bold };
  }
  const css = await (await fetch(CSS_URL, { headers: { "user-agent": "curl/8" } })).text();
  const urls = [...css.matchAll(/url\((https:[^)]+\.ttf)\)/g)].map((m) => m[1]!);

  if (urls.length < 2) {
    throw new Error(`expected 2 ttf urls, got ${urls.length}`);
  }
  await mkdir(DIR, { recursive: true });
  for (const [i, path] of [regular, bold].entries()) {
    await Bun.write(path, await (await fetch(urls[i]!)).arrayBuffer());
  }
  return { regular, bold };
}
