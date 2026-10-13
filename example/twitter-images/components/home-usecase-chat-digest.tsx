export const name = "home-usecase-chat-digest";

export const width = 1200;
export const height = 675;

export const outputs = [{ ratio: 1, format: "png" }] as const;

export const fonts = ["geist/Geist[wght].woff2", "geist/GeistMono[wght].woff2"];

const days = [
  { day: "Mon", messages: 642 },
  { day: "Tue", messages: 811 },
  { day: "Wed", messages: 935 },
  { day: "Thu", messages: 702 },
  { day: "Fri", messages: 588 },
  { day: "Sat", messages: 214 },
  { day: "Sun", messages: 176 },
];

const stats = [
  { label: "Messages", value: "4,068", delta: "+12% on last week" },
  { label: "Threads resolved", value: "183", delta: "of 201 opened" },
  { label: "Median first reply", value: "7m", delta: "was 11m" },
];

const helpers = [
  { handle: "noor", replies: 64, color: "#e0773a" },
  { handle: "kenji.t", replies: 51, color: "#3f8f6b" },
  { handle: "aless", replies: 38, color: "#7a5bd6" },
];

const INK = "#17181c";
const MUTED = "#73767f";
const BAR = "#2f5bea";
const peak = Math.max(...days.map((day) => day.messages));

function BarChart() {
  return (
    <div tw="flex flex-1 flex-col">
      <div tw="flex items-baseline justify-between">
        <span tw="text-[20px] font-semibold">Messages per day</span>
        <span tw="text-[18px]" style={{ color: MUTED }}>
          Peak Wed · 935
        </span>
      </div>
      <div tw="mt-4 flex flex-1 items-end">
        {days.map(({ day, messages }) => (
          <div key={day} tw="flex flex-1 flex-col items-center px-2.5">
            <span
              tw="mb-2 text-[16px]"
              style={{ fontFamily: "Geist Mono", color: messages === peak ? INK : MUTED }}
            >
              {messages}
            </span>
            <div
              tw="w-full rounded-t-[6px]"
              style={{
                height: `${(messages / peak) * 190}px`,
                backgroundColor: messages === peak ? BAR : "#c9d4fb",
              }}
            />
            <span tw="mt-3 text-[16px]" style={{ color: MUTED }}>
              {day}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

export default function ChatDigest() {
  return (
    <div
      tw="flex h-full w-full flex-col bg-[#f7f6f2] px-16 py-12"
      style={{ color: INK, fontFamily: "Geist" }}
    >
      <div tw="flex items-center justify-between">
        <div tw="flex items-baseline">
          <span tw="text-[40px] font-semibold" style={{ letterSpacing: "-0.02em" }}>
            #help-desk
          </span>
          <span tw="ml-4 text-[22px]" style={{ color: MUTED }}>
            Week of Oct 5 – 11, 2026
          </span>
        </div>
        <span
          tw="rounded-full border border-[#e4e2db] bg-white px-4 py-1.5 text-[16px]"
          style={{ color: MUTED }}
        >
          Digestbot · every Monday
        </span>
      </div>

      <div tw="mt-8 flex">
        {stats.map((stat) => (
          <div key={stat.label} tw="flex flex-1 flex-col">
            <span tw="text-[18px]" style={{ color: MUTED }}>
              {stat.label}
            </span>
            <div tw="mt-1 flex items-baseline">
              <span
                tw="text-[48px] font-semibold"
                style={{ fontFamily: "Geist Mono", letterSpacing: "-0.03em" }}
              >
                {stat.value}
              </span>
              <span tw="ml-3 text-[17px]" style={{ color: MUTED }}>
                {stat.delta}
              </span>
            </div>
          </div>
        ))}
      </div>

      <div tw="mt-8 flex flex-1">
        <div tw="flex flex-1 rounded-2xl border border-[#e4e2db] bg-white px-7 py-6">
          <BarChart />
        </div>
        <div tw="ml-5 flex w-[300px] flex-col rounded-2xl border border-[#e4e2db] bg-white px-7 py-6">
          <span tw="text-[20px] font-semibold">Top helpers</span>
          {helpers.map((helper) => (
            <div key={helper.handle} tw="mt-6 flex items-center">
              <div
                tw="flex h-11 w-11 items-center justify-center rounded-full text-[18px] font-semibold text-white"
                style={{ backgroundColor: helper.color }}
              >
                {helper.handle.charAt(0).toUpperCase()}
              </div>
              <span tw="ml-3 flex-1 text-[20px]">@{helper.handle}</span>
              <span tw="text-[18px]" style={{ fontFamily: "Geist Mono", color: MUTED }}>
                {helper.replies}
              </span>
            </div>
          ))}
          <span tw="mt-auto text-[15px]" style={{ color: MUTED }}>
            Replies on threads opened by others
          </span>
        </div>
      </div>
    </div>
  );
}
