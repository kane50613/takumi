import { mkdir } from "node:fs/promises";
import { googleFonts } from "@takumi-rs/helpers";
import { write } from "bun";
import { measure, render } from "takumi-pdf";
import { facturXml, facturXmp, invoice } from "./facturx";
import { InvoiceDocument } from "./invoice";

const ATTACHMENT_NAME = "factur-x.xml";
const PROFILE = "minimum";

await mkdir("output", { recursive: true });

const images = [
  {
    src: "logo.svg",
    data: new Uint8Array(await Bun.file("../../docs/public/logo.svg").arrayBuffer()),
  },
];

const fonts = await googleFonts([{ name: "Inter", weight: [400, 500, 600] }]);

const footer = (
  <div tw="flex w-full justify-between px-12 text-[10px] text-[#6b7280]">
    <span>
      {invoice.seller.name} · {invoice.number}
    </span>
    <span tw="flex">
      Page <span className="pageNumber" /> of <span className="totalPages" />
    </span>
  </div>
);

const { height } = await measure(footer, { size: "a4", fonts });

const startTime = Date.now();

const pdf = await render(<InvoiceDocument data={invoice} />, {
  size: "a4",
  margin: { top: 48, bottom: height + 32, left: 48, right: 48 },
  footer,
  images,
  fonts,
  lang: "en",
  pdfa: "3b",
  tagged: "ua1",
  metadata: {
    title: `Invoice ${invoice.number}`,
    authors: [invoice.seller.name],
    creationDate: invoice.issuedAt,
    xmp: [facturXmp(ATTACHMENT_NAME, PROFILE)],
  },
  attachments: [
    {
      name: ATTACHMENT_NAME,
      data: facturXml(invoice),
      mimeType: "text/xml",
      description: "Factur-X 1.0 MINIMUM invoice data",
      relationship: "data",
    },
  ],
});

console.log(`Rendered output/invoice.pdf in ${Date.now() - startTime}ms`);

await write("output/invoice.pdf", pdf);
