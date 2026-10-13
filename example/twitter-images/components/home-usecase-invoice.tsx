import type { GoogleFontFamily } from "takumi-js/helpers";

export const name = "home-usecase-invoice";

export const title = "Invoice HOS-2026-031";

// A4 portrait at 96 dpi.
export const width = 794;
export const height = 1123;

export const previewRatio = 1.5;

export const googleFonts: GoogleFontFamily[] = [
  { name: "Newsreader", weight: 500 },
  { name: "Inter", weight: [400, 500, 600] },
];

const items = [
  {
    name: "Signage audit",
    detail: "Three ferry terminals, on site",
    quantity: 3,
    unit: "days",
    rate: 720,
  },
  {
    name: "Pictogram set",
    detail: "24 icons, SVG and print masters",
    quantity: 1,
    unit: "set",
    rate: 2880,
  },
  {
    name: "Terminal map illustrations",
    detail: "Cais do Sodré and Cacilhas",
    quantity: 2,
    unit: "maps",
    rate: 1450,
  },
  {
    name: "Wayfinding style guide",
    detail: "36 pages, PDF and InDesign",
    quantity: 1,
    unit: "guide",
    rate: 1600,
  },
  {
    name: "Print-ready files and handoff",
    detail: "Supplier call included",
    quantity: 1,
    unit: "batch",
    rate: 600,
  },
];

const VAT_RATE = 0.23;
const subtotal = items.reduce((sum, item) => sum + item.quantity * item.rate, 0);
const vat = subtotal * VAT_RATE;
const total = subtotal + vat;

const euro = new Intl.NumberFormat("en-IE", { style: "currency", currency: "EUR" });

const INK = "#1d211f";
const MUTED = "#6b726e";
const OAK = "#2f4a3a";
const LINE = "#e3e6e1";
const NUMERIC = { fontVariantNumeric: "tabular-nums" } as const;

function Meta({ label, value }: { label: string; value: string }) {
  return (
    <div tw="flex flex-1 flex-col">
      <span tw="text-[11px]" style={{ color: MUTED }}>
        {label}
      </span>
      <span tw="mt-1 text-[13px] font-medium">{value}</span>
    </div>
  );
}

function Party({ label, lines }: { label: string; lines: string[] }) {
  return (
    <div tw="flex flex-1 flex-col text-[12px] leading-[1.6]">
      <span tw="mb-1 text-[11px]" style={{ color: MUTED }}>
        {label}
      </span>
      {lines.map((line, index) => (
        <span key={line} tw={index === 0 ? "font-semibold" : ""}>
          {line}
        </span>
      ))}
    </div>
  );
}

function Row({ label, value, strong = false }: { label: string; value: string; strong?: boolean }) {
  return (
    <div
      tw={`flex justify-between ${strong ? "mt-3 border-t pt-3 text-[15px] font-semibold" : "mt-2"}`}
      style={{ borderColor: LINE }}
    >
      <span style={{ color: strong ? INK : MUTED }}>{label}</span>
      <span style={NUMERIC}>{value}</span>
    </div>
  );
}

export default function Invoice() {
  return (
    <div
      tw="flex h-full w-full flex-col bg-white px-16 py-14 text-[12px]"
      style={{ color: INK, fontFamily: "Inter" }}
    >
      <div tw="flex items-start justify-between">
        <div tw="flex items-center">
          <div
            tw="flex h-10 w-10 items-center justify-center rounded-[10px]"
            style={{ backgroundColor: OAK }}
          >
            <div tw="h-5 w-5 rounded-full border-[3px] border-white" />
          </div>
          <div tw="ml-3 flex flex-col">
            <span tw="text-[15px] font-semibold">Hollow Oak Studio</span>
            <span style={{ color: MUTED }}>Wayfinding and illustration</span>
          </div>
        </div>
        <div tw="flex flex-col items-end">
          <span tw="text-[34px] leading-none" style={{ fontFamily: "Newsreader", fontWeight: 500 }}>
            Invoice
          </span>
          <span tw="mt-2" style={{ color: MUTED }}>
            HOS-2026-031
          </span>
        </div>
      </div>

      <div tw="mt-10 flex flex-col rounded-[14px] px-7 py-6" style={{ backgroundColor: "#eef2ec" }}>
        <span tw="text-[12px]" style={{ color: OAK }}>
          Amount due
        </span>
        <span
          tw="mt-1 text-[44px] leading-none"
          style={{ fontFamily: "Newsreader", fontWeight: 500, color: OAK, ...NUMERIC }}
        >
          {euro.format(total)}
        </span>
        <div tw="mt-6 flex">
          <Meta label="Issued" value="Oct 14, 2026" />
          <Meta label="Due" value="Nov 13, 2026" />
          <Meta label="Purchase order" value="BF-PO-8812" />
          <Meta label="Project" value="Wayfinding, phase 2" />
        </div>
      </div>

      <div tw="mt-9 flex">
        <Party
          label="From"
          lines={[
            "Hollow Oak Studio, Lda.",
            "Rua das Flores 112",
            "4050-265 Porto, Portugal",
            "NIF PT516204873",
          ]}
        />
        <Party
          label="Bill to"
          lines={[
            "Brightwater Ferries, Lda.",
            "Av. Infante D. Henrique 7",
            "1100-278 Lisboa, Portugal",
            "NIF PT509771450",
          ]}
        />
      </div>

      <div tw="mt-10 flex flex-col">
        <div tw="flex border-b pb-2 text-[11px]" style={{ borderColor: LINE, color: MUTED }}>
          <span tw="flex-1">Description</span>
          <span tw="w-[90px] text-right">Quantity</span>
          <span tw="w-[90px] text-right">Rate</span>
          <span tw="w-[100px] text-right">Amount</span>
        </div>
        {items.map((item) => (
          <div key={item.name} tw="flex border-b py-3" style={{ borderColor: LINE }}>
            <div tw="flex flex-1 flex-col">
              <span tw="font-medium">{item.name}</span>
              <span tw="mt-0.5 text-[11px]" style={{ color: MUTED }}>
                {item.detail}
              </span>
            </div>
            <span tw="w-[90px] text-right" style={{ color: MUTED, ...NUMERIC }}>
              {item.quantity} {item.unit}
            </span>
            <span tw="w-[90px] text-right" style={{ color: MUTED, ...NUMERIC }}>
              {euro.format(item.rate)}
            </span>
            <span tw="w-[100px] text-right" style={NUMERIC}>
              {euro.format(item.quantity * item.rate)}
            </span>
          </div>
        ))}
      </div>

      <div tw="mt-5 flex justify-end">
        <div tw="flex w-[280px] flex-col">
          <Row label="Subtotal" value={euro.format(subtotal)} />
          <Row label="VAT 23%" value={euro.format(vat)} />
          <Row label="Total due" value={euro.format(total)} strong />
        </div>
      </div>

      <div
        tw="mt-auto flex justify-between rounded-[14px] border px-7 py-5"
        style={{ borderColor: LINE }}
      >
        <div tw="flex flex-col leading-[1.6]">
          <span tw="mb-1 text-[11px]" style={{ color: MUTED }}>
            Pay by bank transfer
          </span>
          <span style={NUMERIC}>IBAN PT50 0035 0412 0006 2847 3013 2</span>
          <span>BIC BKOAPTPL · Reference HOS-2026-031</span>
        </div>
        <div tw="flex flex-col items-end leading-[1.6]">
          <span tw="mb-1 text-[11px]" style={{ color: MUTED }}>
            Questions
          </span>
          <span>accounts@hollowoak.studio</span>
          <span>+351 222 018 440</span>
        </div>
      </div>
    </div>
  );
}
