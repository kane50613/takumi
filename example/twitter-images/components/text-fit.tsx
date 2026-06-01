export const name = "text-fit";

export const fonts = [];

export const width = 1200;
export const height = 675;

const Logo = () => (
  <svg width="24" height="24" viewBox="0 0 128 128">
    <path
      fill="black"
      d="M114.3 14.1c1.1.9 3.2 2.7 4.2 4.5s.9 3.5.8 4.8-.4 2.3-2 4.3c-1.7 2-4.7 5-12.8 13.7-8.1 8.8-21.4 23.4-35.3 38.4s-28.6 30.5-36.4 38.7-8.8 8.8-10 9.2-2.5.4-4.1 0-3.5-1.2-6.5-3.8-7.1-6.9-9.4-10S.1 108.8.1 107c0-1.7.4-3.4 5.3-8.6s14.3-13.9 30.5-28.9c16.1-14.9 39-36.1 51-47.2s13.2-12 14.7-12.7 3.2-1.1 4.8-.9 2.8 1 3.9 1.9a32 32 0 0 1 2.5 2.1l.4.5z"
    />
    <path
      fill="black"
      d="M79 .5C65.3 3.1 46.9 23.4 56.8 36.3c3.3 4.3 5.1 6.7 9.3 9.7 10.2 7.3 39.1 31 53.1 26.9 12-3.5 9.4-16.9 5.6-25.8-1.3-3-25.7-52.8-45.8-46.6"
    />
  </svg>
);

export default function TextFit() {
  return (
    <div tw="w-full h-full bg-[#f8fafc] text-slate-900 flex flex-col items-center justify-center">
      <div
        tw="font-semibold leading-[1.2] text-balance w-[85%] font-mono"
        style={{
          textFit: "grow per-line-all",
          whiteSpace: "pre-wrap",
        }}
      >
        <span tw="text-[#f30]">Takumi 1.2 </span>
        <span tw="text-slate-700">
          comes with
          <br />
        </span>
        <span
          tw="text-[#1a6ef5] font-bold bg-slate-400 border-gray-500 rounded-sm p-2"
          style={{ whiteSpace: "nowrap" }}
        >
          text-fit
          <br />
        </span>
        property support
      </div>
      <div tw="mt-12 -mb-12 flex items-center justify-center gap-4 opacity-50 text-2xl font-medium">
        <Logo />
        takumi.kane.tw
      </div>
    </div>
  );
}
