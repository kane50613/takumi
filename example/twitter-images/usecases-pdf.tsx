import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fromJsx } from "takumi-js/helpers/jsx";
import { Renderer } from "takumi-js/node";
import { render } from "takumi-pdf";
import { loadFonts, loadImages } from "./assets";
import * as Certificate from "./components/home-usecase-certificate";
import * as Invoice from "./components/home-usecase-invoice";
import * as Ticket from "./components/home-usecase-ticket";

// Each document renders once as a single-page PDF and once as a WebP preview of the same page.
for (const module of [Ticket, Invoice, Certificate]) {
  const [fonts, images] = await Promise.all([loadFonts(module), loadImages(module)]);

  const pdf = await render(<module.default />, {
    viewport: { width: module.width, height: module.height },
    fonts,
    images,
    metadata: { title: module.title },
  });

  await writeFile(join("output", `${module.name}.pdf`), pdf);

  const { node } = await fromJsx(<module.default />);
  const preview = await new Renderer().render(node, {
    width: module.width * module.previewRatio,
    height: module.height * module.previewRatio,
    devicePixelRatio: module.previewRatio,
    format: "webp",
    quality: 85,
    images,
    fonts,
  });

  await writeFile(join("output", `${module.name}.webp`), preview);
  console.log(
    `${module.name}: pdf ${(pdf.length / 1024).toFixed(0)} KB, webp ${(preview.length / 1024).toFixed(0)} KB`,
  );
}
