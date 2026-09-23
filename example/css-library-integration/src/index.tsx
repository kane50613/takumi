import { mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { write } from "bun";
import { TailwindCard, UnoCard } from "./card";
import { compileTailwindStylesheet } from "./tailwind-compile";
import { compileUnoStylesheet } from "./unocss-compile";
import { render } from "takumi-js";

const width = 1200;
const height = 630;

const sourceDir = dirname(fileURLToPath(import.meta.url));
const outputDir = join(sourceDir, "..", "output");

await mkdir(outputDir, { recursive: true });

const stylesheets = [
  {
    Card: TailwindCard,
    css: await compileTailwindStylesheet(sourceDir),
    imageName: "tailwind-stylesheets.png",
    libraryName: "Tailwind CSS",
    outputName: "tailwind.generated.css",
    title: "Compiled stylesheets",
    description:
      "Tailwind utilities are compiled to CSS, loaded from disk, and applied through Takumi's stylesheet pipeline.",
  },
  {
    Card: UnoCard,
    css: await compileUnoStylesheet(sourceDir),
    imageName: "unocss-stylesheets.png",
    libraryName: "UnoCSS",
    outputName: "unocss.generated.css",
    title: "Generated utilities",
    description:
      "UnoCSS utilities are generated from the same JSX classes and applied through Takumi's stylesheet pipeline.",
  },
];

for (const { Card, css, imageName, libraryName, outputName, title, description } of stylesheets) {
  await write(join(outputDir, outputName), css);

  const image = await render(
    <Card description={description} libraryName={libraryName} title={title} />,
    { width, height, css },
  );

  await write(join(outputDir, imageName), image);
}
