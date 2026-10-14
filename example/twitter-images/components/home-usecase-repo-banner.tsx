export const name = "home-usecase-repo-banner";

export const width = 1280;
export const height = 440;

export const outputs = [] as const;

export const svg = true;

export const fonts = ["geist/Geist[wght].woff2", "geist/GeistMono[wght].woff2"];

const badges = [
  { label: "build", value: "passing", color: "#2f9e5b" },
  { label: "coverage", value: "94%", color: "#2f9e5b" },
  { label: "npm", value: "v2.3.1", color: "#2a6fdb" },
  { label: "postgres", value: "14 – 17", color: "#2a6fdb" },
  { label: "license", value: "MIT", color: "#8a6d1f" },
];

const jobs = [
  { state: "done", job: "send_receipt", detail: "42 ms", color: "#2f9e5b" },
  { state: "done", job: "resize_avatar", detail: "118 ms", color: "#2f9e5b" },
  { state: "retry", job: "deliver_webhook", detail: "2 of 5", color: "#d9822b" },
  { state: "run", job: "sync_invoices", detail: "4.1 s", color: "#2a6fdb" },
  { state: "wait", job: "nightly_export", detail: "in 2 min", color: "#9aa5a2" },
];

const INK = "#10221f";
const MUTED = "#5d6d69";
const TIDE = "#0f766e";

function Mark() {
  return (
    <div
      tw="flex h-[72px] w-[72px] flex-col justify-center rounded-[18px] px-3.5"
      style={{ backgroundColor: TIDE }}
    >
      {[34, 44, 24].map((barWidth, index) => (
        <div
          key={barWidth}
          tw={`h-[8px] rounded-full ${index > 0 ? "mt-[7px]" : ""}`}
          style={{ width: `${barWidth}px`, backgroundColor: index === 1 ? "#99f6e4" : "#ffffff" }}
        />
      ))}
    </div>
  );
}

function Badge({ label, value, color }: (typeof badges)[number]) {
  return (
    <div tw="flex overflow-hidden rounded-[6px] text-[16px] text-white">
      <span tw="bg-[#4a5250] px-2.5 py-1">{label}</span>
      <span tw="px-2.5 py-1 font-semibold" style={{ backgroundColor: color }}>
        {value}
      </span>
    </div>
  );
}

function Queue() {
  return (
    <div
      tw="flex w-[460px] flex-col rounded-2xl border border-[#d5e0dc] bg-white px-6 py-5 text-[17px]"
      style={{ fontFamily: "Geist Mono" }}
    >
      <div tw="mb-2 flex justify-between text-[15px]" style={{ color: MUTED }}>
        <span>queue: default</span>
        <span>5 jobs</span>
      </div>
      {jobs.map((job) => (
        <div key={job.job} tw="mt-2.5 flex items-center">
          <span tw="h-2.5 w-2.5 rounded-full" style={{ backgroundColor: job.color }} />
          <span tw="ml-3 w-[64px]" style={{ color: job.color }}>
            {job.state}
          </span>
          <span tw="flex-1">{job.job}</span>
          <span style={{ color: MUTED }}>{job.detail}</span>
        </div>
      ))}
    </div>
  );
}

export default function RepoBanner() {
  return (
    <div
      tw="flex h-full w-full flex-col justify-between bg-[#eef3f1] px-20 py-16"
      style={{ color: INK, fontFamily: "Geist" }}
    >
      <div tw="flex items-start justify-between">
        <div tw="flex flex-col">
          <div tw="flex items-center">
            <Mark />
            <span tw="ml-6 text-[64px] font-bold" style={{ letterSpacing: "-0.04em" }}>
              tidepool
            </span>
          </div>
          <span
            tw="mt-6 w-[540px] text-[28px] leading-[1.3]"
            style={{ color: MUTED, textWrap: "balance" }}
          >
            A small, durable job queue that lives in your Postgres.
          </span>
          <span tw="mt-6 text-[20px]" style={{ fontFamily: "Geist Mono" }}>
            <span style={{ color: TIDE }}>$</span> npm i tidepool
          </span>
        </div>
        <Queue />
      </div>
      <div tw="flex">
        {badges.map((badge, index) => (
          <div key={badge.label} tw={index > 0 ? "ml-2.5 flex" : "flex"}>
            <Badge {...badge} />
          </div>
        ))}
      </div>
    </div>
  );
}
