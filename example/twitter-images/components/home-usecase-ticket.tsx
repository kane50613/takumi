import type { GoogleFontFamily } from "takumi-js/helpers";

export const name = "home-usecase-ticket";

export const title = "Harbor/26 conference pass";

// 8.5in × 3.5in at 96 dpi.
export const width = 816;
export const height = 336;

export const previewRatio = 2;

export const googleFonts: GoogleFontFamily[] = [
  { name: "Big Shoulders Display", weight: 800 },
  { name: "IBM Plex Mono", weight: 500 },
  { name: "Inter", weight: [500, 700] },
];

const ticketNumber = "HB26-04172";

const fields = [
  { label: "Attendee", value: "Inês Carvalho" },
  { label: "Pass", value: "Full conference" },
  { label: "Dates", value: "Nov 19 – 20, 2026" },
  { label: "Venue", value: "Pier 3 Pavilion, Lisbon" },
];

const PAPER = "#f4efe5";
const INK = "#16213a";
const MUTED = "#6c6a66";
const STUB = "#e4572e";
const STUB_WIDTH = 224;

// Bar widths derived from the ticket number, so the stub is stable across renders.
const barcode = [...`${ticketNumber}${ticketNumber}`].flatMap((char) => {
  const code = char.charCodeAt(0);

  return [1 + (code % 3), 1 + ((code >> 2) % 2)];
});

function Label({ children, color = MUTED }: { children: string; color?: string }) {
  return (
    <span
      tw="text-[11px] uppercase tracking-[0.16em]"
      style={{ fontFamily: "IBM Plex Mono", color }}
    >
      {children}
    </span>
  );
}

function Notch({ top }: { top: number }) {
  return (
    <div
      tw="absolute h-6 w-6 rounded-full bg-white"
      style={{ top: `${top}px`, right: `${STUB_WIDTH - 12}px` }}
    />
  );
}

export default function Ticket() {
  return (
    <div
      tw="relative flex h-full w-full overflow-hidden"
      style={{ backgroundColor: PAPER, color: INK, fontFamily: "Inter" }}
    >
      <div tw="flex flex-1 flex-col justify-between px-11 py-9">
        <div tw="flex flex-col">
          <Label>Systems conference · Lisbon</Label>
          <span
            tw="mt-3 text-[96px] leading-[0.8]"
            style={{ fontFamily: "Big Shoulders Display", fontWeight: 800 }}
          >
            HARBOR/26
          </span>
        </div>
        <div tw="flex flex-wrap">
          {fields.map((field) => (
            <div key={field.label} tw="mt-4 flex w-1/2 flex-col">
              <Label>{field.label}</Label>
              <span tw="mt-1 text-[19px] font-medium">{field.value}</span>
            </div>
          ))}
        </div>
      </div>

      <div
        tw="flex flex-col justify-between px-7 py-9 text-white"
        style={{
          width: `${STUB_WIDTH}px`,
          backgroundColor: STUB,
          borderLeft: `2px dashed ${PAPER}`,
        }}
      >
        <div tw="flex flex-col">
          <Label color="#ffe1d3">Admit one</Label>
          <span tw="mt-2 text-[30px] font-bold leading-[1.05]">Doors 08:30</span>
          <span tw="mt-2 text-[14px] font-medium">Day 1 opens in Hall B</span>
        </div>
        <div tw="flex flex-col">
          <div tw="flex h-[64px] items-stretch rounded-[6px] bg-white px-2.5 py-2">
            {barcode.map((barWidth, index) => (
              <div
                key={index}
                tw="h-full"
                style={{
                  flexGrow: barWidth,
                  backgroundColor: index % 2 === 0 ? INK : "transparent",
                }}
              />
            ))}
          </div>
          <span tw="mt-2 text-[13px]" style={{ fontFamily: "IBM Plex Mono" }}>
            {ticketNumber}
          </span>
        </div>
      </div>

      <Notch top={-12} />
      <Notch top={height - 12} />
    </div>
  );
}
