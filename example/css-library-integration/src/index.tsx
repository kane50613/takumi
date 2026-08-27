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

const currentFile = fileURLToPath(import.meta.url);
const currentDir = dirname(currentFile);
const exampleDir = dirname(currentFile);
const outputDir = join(exampleDir, "..", "output");

await mkdir(outputDir, { recursive: true });

const stylesheets = [
  {
    css: await compileTailwindStylesheet(currentDir),
    imageName: "tailwind-stylesheets.png",
    libraryName: "Tailwind CSS",
    outputName: "tailwind.generated.css",
    title: "Compiled stylesheets",
    description:
      "Tailwind utilities are compiled to CSS, loaded from disk, and applied through Takumi's stylesheet pipeline.",
  },
  {
    css: await compileUnoStylesheet(currentDir),
    imageName: "unocss-stylesheets.png",
    libraryName: "UnoCSS",
    outputName: "unocss.generated.css",
    title: "Generated utilities",
    description:
      "UnoCSS utilities are generated from the same JSX classes and applied through Takumi's stylesheet pipeline.",
  },
] as const;

for (const stylesheet of stylesheets) {
  await write(join(outputDir, stylesheet.outputName), stylesheet.css);

  const CardComponent = stylesheet.libraryName === "Tailwind CSS" ? TailwindCard : UnoCard;

  const image = await render(
    <CardComponent
      description={stylesheet.description}
      libraryName={stylesheet.libraryName}
      title={stylesheet.title}
    />,
    {
      width,
      height,
      css: stylesheet.css,
    },
  );

  await write(join(outputDir, stylesheet.imageName), image);
}
