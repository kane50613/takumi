import { expect, test } from "bun:test";
import { mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { renderToStaticMarkup } from "react-dom/server";
import { Hero } from "../app/components/home/hero";
import { Features } from "../app/components/home/features";
import { Filmstrip } from "../app/components/home/filmstrip";

const docsRoot = resolve(import.meta.dirname, "..");

test("homepage copy renders with its links and image descriptions", () => {
  const basePath = process.env.WAKU_CONFIG_BASE_PATH;

  process.env.WAKU_CONFIG_BASE_PATH = "/";

  const html = renderToStaticMarkup(
    <>
      <Hero />
      <Features />
      <Filmstrip />
    </>,
  );

  if (basePath === undefined) delete process.env.WAKU_CONFIG_BASE_PATH;
  else process.env.WAKU_CONFIG_BASE_PATH = basePath;

  expect(html.replace(/ class="[^"]*"/g, "").replace(/ style="[^"]*"/g, "")).toMatchSnapshot();
});

test("guides have titles and distinct descriptions for discovery", async () => {
  const pages = [];
  const descriptions = new Set<string>();

  for (const path of Array.from(
    new Bun.Glob("**/*.mdx").scanSync(join(docsRoot, "content/docs")),
  ).sort()) {
    const source = await readFile(join(docsRoot, "content/docs", path), "utf8");
    const title = source.match(/^title: (.+)$/m)?.[1];
    const description = source.match(/^description: (.+)$/m)?.[1];

    expect(title).toBeTruthy();
    expect(description).toBeTruthy();
    if (!description) throw new Error(`Missing description in ${path}`);
    expect(descriptions.has(description)).toBe(false);
    descriptions.add(description);
    pages.push({ path, title, description });
  }

  expect(pages).toMatchSnapshot();
});

test("README quick starts execute and produce the expected PNG and PDF", async () => {
  const readme = await readFile(join(docsRoot, "../README.md"), "utf8");
  const examples = Array.from(readme.matchAll(/```tsx\n([\s\S]*?)```/g)).slice(0, 2);
  const directory = await mkdtemp(join(tmpdir(), "takumi-readme-"));

  expect(examples).toHaveLength(2);

  try {
    await symlink(join(docsRoot, "node_modules"), join(directory, "node_modules"), "dir");
    for (const [index, example] of examples.entries()) {
      const code = example[1];

      if (!code) throw new Error(`Missing README example ${index}`);
      const script = join(directory, `${index}.tsx`);

      await writeFile(script, code);
      const process = Bun.spawn([Bun.which("bun") ?? "bun", script], {
        cwd: directory,
        stdout: "pipe",
        stderr: "pipe",
      });
      const [exitCode, errors] = await Promise.all([
        process.exited,
        new Response(process.stderr).text(),
      ]);

      expect(errors).toBe("");
      expect(exitCode).toBe(0);
    }

    for (const name of ["output.png", "invoice.pdf"]) {
      const output = await readFile(join(directory, name));
      const golden = await readFile(join(import.meta.dirname, "fixtures/readme", name));

      expect(output.equals(golden)).toBe(true);
    }
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
