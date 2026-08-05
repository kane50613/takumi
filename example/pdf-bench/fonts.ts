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
  const cssResponse = await fetch(CSS_URL, { headers: { "user-agent": "curl/8" } });

  if (!cssResponse.ok) {
    throw new Error(`font css request failed: ${cssResponse.status}`);
  }
  const css = await cssResponse.text();
  const urls = [...css.matchAll(/url\((https:[^)]+\.ttf)\)/g)].map((m) => m[1]!);

  if (urls.length < 2) {
    throw new Error(`expected 2 ttf urls, got ${urls.length}`);
  }
  await mkdir(DIR, { recursive: true });
  for (const [i, path] of [regular, bold].entries()) {
    const fontResponse = await fetch(urls[i]!);

    if (!fontResponse.ok) {
      throw new Error(`font download failed: ${fontResponse.status}`);
    }
    await Bun.write(path, await fontResponse.arrayBuffer());
  }
  return { regular, bold };
}
