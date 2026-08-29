const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="760" height="420" viewBox="0 0 760 420">
  <defs>
    <path id="arc" d="M 90 380 A 290 290 0 0 1 670 380" fill="none" />
  </defs>
  <circle cx="380" cy="300" r="200" fill="none" stroke="#1d4ed8" stroke-width="2" />
  <text font-family="Noto Sans" font-size="46" font-weight="700" fill="#1d4ed8">
    <textPath href="#arc" startOffset="50%" text-anchor="middle">Text on a path</textPath>
  </text>
  <text x="380" y="290" font-family="Noto Sans" font-size="120" font-weight="800" fill="#0f172a" text-anchor="middle">SVG</text>
  <text x="380" y="340" font-family="Noto Sans" font-size="28" fill="#475569" text-anchor="middle">
    <tspan>drawn from</tspan><tspan dx="8" fill="#1d4ed8">registered fonts</tspan>
  </text>
</svg>`;

export default function SvgText() {
  return (
    <div tw="flex h-full w-full items-center justify-center bg-white">
      <img src={`data:image/svg+xml;utf8,${encodeURIComponent(svg)}`} width={760} height={420} />
    </div>
  );
}

export const options: PlaygroundOptions = {
  width: 1200,
  height: 630,
  format: "png",
};
