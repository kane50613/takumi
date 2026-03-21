import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { type OutputFormat, Renderer } from "@takumi-rs/core";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import * as FiveHundredStars from "./components/500-stars";
import * as GithubSocialPreview from "./components/github-social-preview";
import * as OgImage from "./components/og-image";
import * as PackageOgImage from "./components/package-og-image";
import * as PrismaOGImage from "./components/prisma-og-image";
import * as XPostImage from "./components/x-post-image";

const components = [
  OgImage,
  FiveHundredStars,
  XPostImage,
  PrismaOGImage,
  PackageOgImage,
  GithubSocialPreview,
];

type Component = (typeof components)[number];

async function createRenderer(module: Component) {
  return new Renderer({
    persistentImages: module.persistentImages,
    fonts:
      module.fonts.length > 0
        ? await Promise.all(module.fonts.map((font) => readFile(join("../../assets/fonts", font))))
        : undefined,
  });
}

async function render(
  renderer: Renderer,
  module: Component,
  ratio = 1,
  format: OutputFormat = "png",
) {
  const jsxPrepareStart = performance.now();
  const { node, stylesheets } = await fromJsx(<module.default />);
  const renderStart = performance.now();

  const buffer = await renderer.render(node, {
    width: module.width * ratio,
    height: module.height * ratio,
    devicePixelRatio: ratio,
    stylesheets,
    drawDebugBorder: process.argv.includes("--debug"),
    format,
  });

  const end = performance.now();
  const jsxPrepareMs = Math.round(renderStart - jsxPrepareStart);
  const renderMs = Math.round(end - renderStart);
  const totalMs = Math.round(end - jsxPrepareStart);

  console.log(
    `Rendered ${module.name} ${ratio}x in ${totalMs}ms (jsx prepare: ${jsxPrepareMs}ms, render: ${renderMs}ms)`,
  );

  const fileName = ratio === 1 ? `${module.name}.${format}` : `${module.name}@${ratio}x.${format}`;

  await writeFile(join("output", fileName), buffer);
}

for (const component of components) {
  const rendererPrepareStart = performance.now();
  const renderer = await createRenderer(component);
  const rendererPrepareMs = Math.round(performance.now() - rendererPrepareStart);

  console.log(`Prepared ${component.name} renderer in ${rendererPrepareMs}ms`);

  await render(renderer, component);
  await render(renderer, component, 2, "webp");
}
