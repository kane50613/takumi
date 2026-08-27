export const name = "home-filmstrip";

export const fonts = [];

export const width = 480;
export const height = 480;

// One frame per timestamp; the docs homepage lays them out as a film strip.
export const timestamps = [0, 125, 250, 375, 500, 625, 750, 875];

export const css = [
  `@keyframes morph {
    from { border-radius: 12%; transform: rotate(0deg) scale(1); }
    50% { border-radius: 50%; transform: rotate(90deg) scale(0.72); }
    to { border-radius: 12%; transform: rotate(180deg) scale(1); }
  }`,
];

export default function Filmstrip() {
  return (
    <div tw="flex h-full w-full items-center justify-center bg-[#16130f]">
      <div
        tw="h-70 w-70 bg-[#ff4d4d]"
        style={{ animation: "morph 1000ms cubic-bezier(0.65, 0, 0.35, 1) infinite" }}
      />
    </div>
  );
}
