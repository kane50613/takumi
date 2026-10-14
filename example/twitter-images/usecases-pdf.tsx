import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { render } from "takumi-pdf";
import { loadFonts, loadImages } from "./assets";
import * as Certificate from "./components/home-usecase-certificate";
import * as Invoice from "./components/home-usecase-invoice";
import * as Ticket from "./components/home-usecase-ticket";

// Each document renders as a single-page PDF; `index.tsx` renders its SVG preview.
for (const module of [Ticket, Invoice, Certificate]) {
  const [fonts, images] = await Promise.all([loadFonts(module), loadImages(module)]);

  const pdf = await render(<module.default />, {
    viewport: { width: module.width, height: module.height },
    fonts,
    images,
    metadata: { title: module.title },
  });

  await writeFile(join("output", `${module.name}.pdf`), pdf);

  console.log(`${module.name}: pdf ${(pdf.length / 1024).toFixed(0)} KB`);
}
