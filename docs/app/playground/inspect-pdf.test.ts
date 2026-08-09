import { expect, test } from "bun:test";
import { inspectPdf } from "./inspect-pdf";

/** Hand-written so the streams stay uncompressed and the offsets stay readable. */
function pdf(objects: string[]): Uint8Array {
  return new TextEncoder().encode(
    `%PDF-1.7\n${objects.map((object, index) => `${index + 1} 0 obj\n${object}\nendobj\n`).join("")}`,
  );
}

const CONTENT = "<</Length 24>>\nstream\nBT /f0 12 Tf (hi) Tj ET\nendstream";

test("counts a shared content stream against both pages", async () => {
  const inspection = await inspectPdf(
    pdf([
      "<</Type/Catalog/Pages 2 0 R>>",
      "<</Type/Pages/Count 2/Kids[3 0 R 4 0 R]>>",
      "<</Type/Page/Parent 2 0 R/Contents 5 0 R>>",
      "<</Type/Page/Parent 2 0 R/Contents 5 0 R>>",
      CONTENT,
    ]),
  );

  expect(inspection.textObjects).toEqual([1, 1]);
  expect(inspection.objects.find((object) => object.number === "5")?.label).toBe(
    "content stream, pages 1, 2",
  );
});

test("names the single page a content stream belongs to", async () => {
  const inspection = await inspectPdf(
    pdf([
      "<</Type/Catalog/Pages 2 0 R>>",
      "<</Type/Pages/Count 1/Kids[3 0 R]>>",
      "<</Type/Page/Parent 2 0 R/Contents 4 0 R>>",
      CONTENT,
    ]),
  );

  expect(inspection.textObjects).toEqual([1]);
  expect(inspection.objects.find((object) => object.number === "4")?.label).toBe(
    "content stream, page 1",
  );
});
