import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { Renderer } from "takumi-js/node";
import { fromJsx } from "takumi-js/helpers/jsx";
import { googleFonts } from "takumi-js/helpers";

// Logical width; rendered straight to 1080p (devicePixelRatio keeps it crisp, no supersample).
const W = 1600;
const OUT_W = 1920;
const OUT_H = 1080;
const DPR = OUT_W / W;
const FPS = 60;
const SEG_MS = 1150;
const INK = "#1d1d1f";
const MUTED = "#86868b";
const UI = "Inter";
const UI_TEXT = "On-demand Google Fonts, rendered without a browser";

// A line of well-known verse per language, each in a Google Font chosen to suit the verse's
// mood — brush scripts for the East-Asian classics, literary serifs for the Western ones.
const segments = [
  { text: "To be, or not to be", family: "Playfair Display", weight: 500, size: 120 },
  { text: "床前明月光", family: "Ma Shan Zheng", weight: 400, size: 208 },
  { text: "古池や蛙飛びこむ", family: "Yuji Syuku", weight: 400, size: 150 },
  { text: "별 헤는 밤", family: "Nanum Myeongjo", weight: 700, size: 176 },
  { text: "Я вас любил", family: "Lora", weight: 500, size: 152 },
  { text: "سجِّل أنا عربي", family: "Amiri", weight: 400, size: 156 },
  { text: "सारे जहाँ से अच्छा", family: "Rozha One", weight: 400, size: 132 },
  { text: "ความรักเหมือนโรคา", family: "Mali", weight: 500, size: 128 },
  { text: "Nel mezzo del cammin", family: "Cormorant", weight: 600, size: 144 },
  { text: "Navegar é preciso", family: "Cinzel", weight: 500, size: 116 },
  { text: "Trăm năm trong cõi người ta", family: "Playfair Display", weight: 500, size: 96 },
];

const stylesheets = [
  `@keyframes slide {
    0% { opacity: 0; transform: translateY(34px) scale(0.99); }
    24% { opacity: 1; transform: translateY(0) scale(1); }
    76% { opacity: 1; transform: translateY(0) scale(1); }
    100% { opacity: 0; transform: translateY(-34px) scale(0.99); }
  }`,
];

const logo = await readFile(join("../../assets/images/takumi.svg"));

function still(seg: (typeof segments)[number]) {
  const gf = `${seg.family}, sans-serif`;
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
        padding: "72px 96px",
        backgroundColor: "#f5f5f7",
        backgroundImage: "radial-gradient(120% 90% at 50% 0%, #ffffff 0%, #ededf0 100%)",
        color: INK,
        fontFamily: `${UI}, sans-serif`,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "16px" }}>
          <img src="takumi.svg" alt="" style={{ width: "44px", height: "44px" }} />
          <span style={{ fontSize: "34px", fontWeight: 600, letterSpacing: "-0.02em" }}>
            Takumi
          </span>
        </div>
        <span style={{ fontSize: "26px", fontWeight: 600, color: MUTED, letterSpacing: "0.04em" }}>
          On-demand Google Fonts
        </span>
      </div>

      <div
        style={{
          display: "flex",
          flex: 1,
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          gap: "36px",
          animation: `slide ${SEG_MS}ms cubic-bezier(0.4, 0, 0.2, 1) both`,
        }}
      >
        <span
          style={{
            fontFamily: gf,
            fontWeight: seg.weight,
            fontSize: `${seg.size}px`,
            lineHeight: 1,
          }}
        >
          {seg.text}
        </span>
        <span
          style={{
            fontSize: "30px",
            fontWeight: 600,
            color: MUTED,
            letterSpacing: "0.1em",
            textTransform: "uppercase",
          }}
        >
          {seg.family}
        </span>
      </div>

      <div style={{ display: "flex", justifyContent: "center" }}>
        <span style={{ fontSize: "30px", fontWeight: 600, color: MUTED }}>{UI_TEXT}</span>
      </div>
    </div>
  );
}

// Pipe raw RGBA frames straight to ffmpeg — no PNG encode, no disk.
const ff = Bun.spawn(
  [
    "ffmpeg",
    "-y",
    "-f",
    "rawvideo",
    "-pixel_format",
    "rgba",
    "-video_size",
    `${OUT_W}x${OUT_H}`,
    "-framerate",
    String(FPS),
    "-i",
    "-",
    "-r",
    String(FPS),
    "-crf",
    "18",
    "-pix_fmt",
    "yuv420p",
    "-c:v",
    "libx264",
    "-movflags",
    "+faststart",
    "output/google-fonts-showcase.mp4",
  ],
  { stdin: "pipe", stdout: "ignore", stderr: "ignore" },
);

const framesPerSeg = Math.round((FPS * SEG_MS) / 1000);

for (const seg of segments) {
  const { node } = await fromJsx(still(seg));
  const fonts = await googleFonts([
    { family: seg.family, weight: seg.weight },
    { family: UI, weight: 600 },
  ]);
  const renderer = new Renderer();
  for (let f = 0; f < framesPerSeg; f++) {
    const buf = await renderer.render(node, {
      width: OUT_W,
      height: OUT_H,
      devicePixelRatio: DPR,
      format: "raw",
      stylesheets,
      images: [{ src: "takumi.svg", data: logo }],
      fonts,
      timeMs: Math.round((f * 1000) / FPS),
    });
    ff.stdin.write(buf);
    await ff.stdin.flush();
  }
  console.log(`seg ${seg.text} (${seg.family})`);
}

ff.stdin.end();
await ff.exited;
console.log("mp4 done");
