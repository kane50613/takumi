import { PageNumber, TotalPages } from "takumi-pdf/primitives";

const clauses = [
  {
    title: "1. Confidential information",
    body: "Each party may disclose material it treats as confidential. The receiving party protects it with the same care it applies to its own material of like importance, and never with less than reasonable care.",
  },
  {
    title: "2. Permitted use",
    body: "The receiving party uses the material only to evaluate the transaction described in Schedule A. Any other use, including training a model or benchmarking against a competing product, needs written permission.",
  },
  {
    title: "3. Return and destruction",
    body: "On written request the receiving party returns or destroys the material within thirty days, and confirms in writing that it did. Backups made in the ordinary course may be kept until they expire.",
  },
  {
    title: "4. Term",
    body: "These obligations run for three years from the last disclosure. Material that qualifies as a trade secret stays protected for as long as it qualifies.",
  },
];

const parties = ["Kiln Studio Ltd.", "Aomori Systems K.K."];

const TILE_ROWS = 7;
const TILE_COLUMNS = 4;

function TiledWatermark({ label }: { label: string }) {
  return (
    <div
      tw="fixed inset-0 flex flex-col justify-around"
      style={{ zIndex: -1, transform: "rotate(-30deg) scale(1.45)" }}
    >
      {Array.from({ length: TILE_ROWS }, (_, row) => (
        <div key={row} tw="flex justify-around">
          {Array.from({ length: TILE_COLUMNS }, (_, column) => (
            <span
              key={column}
              tw="text-[12px] font-semibold tracking-[0.3em]"
              style={{ color: "rgba(17,24,39,0.06)" }}
            >
              {label}
            </span>
          ))}
        </div>
      ))}
    </div>
  );
}

function Heading() {
  return (
    <div tw="flex flex-col">
      <h1 tw="mt-0 mb-1 text-2xl font-bold">Mutual non-disclosure agreement</h1>
      <span tw="text-xs text-[#6b7280]">
        Between {parties[0]} and {parties[1]} · 2026-08-14
      </span>
    </div>
  );
}

function Clause({ title, body }: { title: string; body: string }) {
  return (
    <div tw="mt-6 flex flex-col break-inside-avoid">
      <h2 tw="m-0 text-base font-semibold">{title}</h2>
      <p tw="mt-2 mb-0 text-sm leading-6 text-[#374151]">{body}</p>
    </div>
  );
}

function Signature({ party }: { party: string }) {
  return (
    <div tw="flex w-[45%] flex-col border-t border-[#9ca3af] pt-2 text-xs">
      <span tw="font-semibold">{party}</span>
      <span tw="text-[#6b7280]">Name, title, date</span>
    </div>
  );
}

function Footer() {
  return (
    <div tw="flex w-full justify-between px-14 pb-5 text-[10px] text-[#9ca3af]">
      <span>Confidential · draft for review</span>
      <span>
        <PageNumber /> / <TotalPages />
      </span>
    </div>
  );
}

export default function Agreement() {
  return (
    <div tw="flex w-full flex-col text-[#111827]">
      <TiledWatermark label="CONFIDENTIAL" />
      <Heading />
      {clauses.map((clause) => (
        <Clause key={clause.title} title={clause.title} body={clause.body} />
      ))}
      <div tw="mt-12 flex justify-between">
        {parties.map((party) => (
          <Signature key={party} party={party} />
        ))}
      </div>
    </div>
  );
}

export const options: PlaygroundOptions = {
  pdf: {
    size: "a4",
    margin: { top: 64, right: 56, bottom: 64, left: 56 },
    footer: <Footer />,
    outline: true,
    metadata: { title: "Mutual non-disclosure agreement", creationDate: "2026-08-14" },
  },
};
