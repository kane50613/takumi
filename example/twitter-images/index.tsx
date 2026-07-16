import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { type OutputFormat } from "takumi-js";
import { Renderer } from "takumi-js/node";
import { fromJsx } from "takumi-js/helpers/jsx";
import { googleFonts } from "takumi-js/helpers";
import * as FiveHundredStars from "./components/500-stars";
import * as GithubSocialPreview from "./components/github-social-preview";
import * as OgImage from "./components/og-image";
import * as PackageOgImage from "./components/package-og-image";
import * as PrismaOGImage from "./components/prisma-og-image";
import * as XPostImage from "./components/x-post-image";
import * as V1 from "./components/v1";
import * as TextFit from "./components/text-fit";
import * as HomeDemoCard from "./components/home-demo-card";
import * as HomeFilmstrip from "./components/home-filmstrip";
import * as GoogleFontsShowcase from "./components/google-fonts-showcase";
import * as DitherOg from "./components/dither-og";

const components = [
  TextFit,
  V1,
  OgImage,
  FiveHundredStars,
  XPostImage,
  PrismaOGImage,
  PackageOgImage,
  GithubSocialPreview,
  HomeDemoCard,
  HomeFilmstrip,
  GoogleFontsShowcase,
  DitherOg,
];

type Component = (typeof components)[number];

function fontLoaders(module: Component) {
  return module.fonts.map((font) => {
    const { path, ...details } = typeof font === "string" ? { path: font } : font;
    return {
      key: path,
      data: () => readFile(join("../../assets/fonts", path)),
      ...details,
    };
  });
}

async function render(
  module: Component,
  ratio = 1,
  format: OutputFormat = "png",
  timeMs?: number,
  frameIndex?: number,
) {
  // Fresh renderer per component: registered fonts persist on a renderer, so a
  // shared one would leak each component's fonts into the next render's
  // default font stack.
  const renderer = new Renderer();
  const jsxPrepareStart = performance.now();
  const { node, stylesheets } = await fromJsx(<module.default />);
  const images = await Promise.all(
    ("images" in module ? module.images : []).map(async ({ src, path }) => ({
      src,
      data: await readFile(join("../../assets/images", path)),
    })),
  );
  const loaders = fontLoaders(module);
  const subsets = "googleFonts" in module ? await googleFonts(module.googleFonts) : [];
  const fonts = [...loaders, ...subsets];
  const renderStart = performance.now();

  const buffer = await renderer.render(node, {
    width: module.width * ratio,
    height: module.height * ratio,
    devicePixelRatio: ratio,
    stylesheets: [...stylesheets, ...("stylesheets" in module ? module.stylesheets : [])],
    drawDebugBorder: process.argv.includes("--debug"),
    images,
    fonts: fonts.length > 0 ? fonts : undefined,
    format,
    timeMs,
  });

  const end = performance.now();
  const jsxPrepareMs = Math.round(renderStart - jsxPrepareStart);
  const renderMs = Math.round(end - renderStart);
  const totalMs = Math.round(end - jsxPrepareStart);

  console.log(
    `Rendered ${module.name} ${ratio}x in ${totalMs}ms (jsx prepare: ${jsxPrepareMs}ms, render: ${renderMs}ms)`,
  );

  const frameSuffix = frameIndex === undefined ? "" : `-${frameIndex}`;
  const ratioSuffix = ratio === 1 ? "" : `@${ratio}x`;

  await writeFile(join("output", `${module.name}${frameSuffix}${ratioSuffix}.${format}`), buffer);
}

for (const component of components) {
  if ("timestamps" in component) {
    for (const [index, timeMs] of component.timestamps.entries()) {
      await render(component, 1, "webp", timeMs, index);
    }
    continue;
  }

  await render(component);
  await render(component, 2, "webp");
}
