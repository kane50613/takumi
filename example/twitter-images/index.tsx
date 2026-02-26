import { join } from "node:path";
import { Renderer } from "@takumi-rs/core";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import { file, write } from "bun";
import * as FiveHundredStars from "./components/500-stars";
import * as OgImage from "./components/og-image";
import * as PrismaOGImage from "./components/prisma-og-image";
import * as XPostImage from "./components/x-post-image";

const components = [OgImage, FiveHundredStars, XPostImage, PrismaOGImage];

type Component = (typeof components)[number];

async function render(module: Component) {
  const { node, stylesheets } = await fromJsx(<module.default />);

  const prepareStart = performance.now();
  const renderer = new Renderer({
    persistentImages: module.persistentImages,
    fonts:
      module.fonts.length > 0
        ? await Promise.all(
            module.fonts.map((font) =>
              file(join("../../assets/fonts", font)).arrayBuffer(),
            ),
          )
        : undefined,
  });

  const renderStart = performance.now();

  const buffer = await renderer.render(node, {
    width: module.width,
    height: module.height,
    stylesheets,
    drawDebugBorder: process.argv.includes("--debug"),
  });

  const end = performance.now();

  console.log(
    `Rendered ${module.name} in ${Math.round(end - prepareStart)}ms (prepare: ${Math.round(renderStart - prepareStart)}ms, render: ${Math.round(end - renderStart)}ms)`,
  );

  await write(join("output", `${module.name}.png`), buffer.buffer);
}

for (const component of components) {
  await render(component);
}
