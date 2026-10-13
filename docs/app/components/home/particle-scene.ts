/**
 * The homepage's one WebGL moment: the headline's ink breaks into particles that fly onto the
 * card, shade into its layout boxes, lift apart as paint layers, and settle as its pixels. World
 * units are the stage's CSS pixels on the z = 0 plane, centered, with y up.
 */
import {
  CanvasTexture,
  Color,
  Euler,
  GLSL3,
  InstancedBufferAttribute,
  InstancedBufferGeometry,
  Matrix3,
  Matrix4,
  Mesh,
  PerspectiveCamera,
  PlaneGeometry,
  Scene,
  ShaderMaterial,
  SRGBColorSpace,
  Vector2,
  WebGLRenderer,
} from "three";
import { ramp, STAGE } from "./stage-timeline";

const FOV = 30;
const LAYER_GAP = 150;
/** The headline is sampled on a grid this many CSS pixels apart. */
const INK_STEP = 2;

export type StageData = {
  image: string;
  width: number;
  height: number;
  boxes: { depth: number; x: number; y: number; width: number; height: number }[];
};

export type Palette = { foreground: string; background: string };

/** The headline's ink and the card's box, in CSS pixels from the stage's top left. */
export type StageLayout = {
  width: number;
  height: number;
  /** `x, y` pairs, ordered left to right. */
  ink: Float32Array;
  card: { left: number; top: number; width: number; height: number };
};

export type ParticleScene = {
  render: (progress: number) => void;
  layout: (layout: StageLayout) => void;
  setPalette: (palette: Palette) => void;
  dispose: () => void;
};

/**
 * Every inked point of `heading`'s text, `INK_STEP` pixels apart, relative to `origin`. Each
 * character is drawn again in the box the browser laid it out in, so the points match the text.
 */
export function sampleInk(heading: HTMLElement, origin: DOMRect) {
  const bounds = heading.getBoundingClientRect();
  const style = getComputedStyle(heading);
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d", { willReadFrequently: true });

  canvas.width = Math.ceil(bounds.width);
  canvas.height = Math.ceil(bounds.height);
  if (!context || canvas.width === 0 || canvas.height === 0) {
    return new Float32Array();
  }
  context.font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
  context.textBaseline = "alphabetic";

  const metrics = context.measureText("Hg");
  const ascent = metrics.fontBoundingBoxAscent;
  const descent = metrics.fontBoundingBoxDescent;
  const walker = document.createTreeWalker(heading, NodeFilter.SHOW_TEXT);
  const range = document.createRange();

  for (let text = walker.nextNode(); text; text = walker.nextNode()) {
    const value = text.textContent ?? "";

    for (let index = 0; index < value.length; index++) {
      range.setStart(text, index);
      range.setEnd(text, index + 1);

      const box = range.getBoundingClientRect();

      if (box.width === 0) {
        continue;
      }
      context.fillText(
        value[index] ?? "",
        box.left - bounds.left,
        box.top - bounds.top + (box.height - ascent - descent) / 2 + ascent,
      );
    }
  }

  const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
  const points: number[] = [];

  for (let x = 0; x < canvas.width; x += INK_STEP) {
    for (let y = 0; y < canvas.height; y += INK_STEP) {
      if ((pixels[(y * canvas.width + x) * 4 + 3] ?? 0) > 110) {
        points.push(bounds.left - origin.left + x, bounds.top - origin.top + y);
      }
    }
  }

  return new Float32Array(points);
}

const easing = (name: string, [from, to]: readonly [number, number]) =>
  `float ${name}() { return smoothstep(0.0, 1.0, clamp((progress - ${from.toFixed(3)}) / ${(to - from).toFixed(3)}, 0.0, 1.0)); }`;

const VERTEX = `
uniform float progress;
uniform vec2 stage;
uniform vec2 cardCenter;
uniform vec2 cardSize;
uniform vec2 grid;
uniform mat3 tilt;
uniform sampler2D map;
uniform float lod;
uniform vec3 foreground;
uniform vec3 background;
in vec2 source;
in vec2 cell;
in float shade;
in float layer;
in float seed;
in float delay;
out vec3 tint;
${easing("layoutShade", STAGE.layout)}
${easing("paintColor", STAGE.paint)}
${easing("separate", STAGE.separate)}
${easing("merge", STAGE.merge)}
${easing("fill", STAGE.fill)}
float inOut(float t) {
  return t < 0.5 ? 4.0 * t * t * t : 1.0 - pow(-2.0 * t + 2.0, 3.0) / 2.0;
}
void main() {
  float start = ${STAGE.flyStart.toFixed(3)} + delay * ${STAGE.flySpread.toFixed(3)};
  float fly = clamp((progress - start) / ${STAGE.flyDuration.toFixed(3)}, 0.0, 1.0);
  float t = inOut(fly);
  vec3 from = vec3(source.x - stage.x * 0.5, stage.y * 0.5 - source.y, 0.0);
  vec2 cellSize = cardSize / grid;
  vec3 local = vec3(
    (cell.x + 0.5) * cellSize.x - cardSize.x * 0.5,
    cardSize.y * 0.5 - (cell.y + 0.5) * cellSize.y,
    layer * ${LAYER_GAP.toFixed(1)} * separate() * (1.0 - merge())
  );
  vec3 landed = vec3(cardCenter, 0.0) + tilt * local;
  vec3 arc = vec3(
    (seed - 0.5) * 220.0,
    (fract(seed * 7.13) - 0.5) * 160.0,
    120.0 + 300.0 * fract(seed * 3.71)
  );
  vec3 control = mix(from, landed, 0.5) + arc;
  vec3 travelled = mix(mix(from, control, t), mix(control, landed, t), t);
  float morph = smoothstep(0.0, 0.55, fly);
  vec2 size = mix(vec2(${(INK_STEP * 1.2).toFixed(2)}), cellSize * mix(0.78, 1.02, fill()), morph);
  vec3 corner = vec3(position.xy * size, 0.0);
  corner = mix(corner, tilt * corner, morph);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(travelled + corner, 1.0);

  vec2 cellUv = (cell + 0.5) / grid;
  vec3 pixel = textureLod(map, vec2(cellUv.x, 1.0 - cellUv.y), lod).rgb;
  vec3 boxes = mix(background, foreground, shade);
  tint = mix(foreground, boxes, layoutShade() * morph);
  tint = mix(tint, pixel, paintColor() * morph);
}`;

const FRAGMENT = `
in vec3 tint;
out vec4 fragment;
void main() {
  fragment = linearToOutputTexel(vec4(tint, 1.0));
}`;

async function loadImage(src: string) {
  const image = new Image();

  image.src = src;
  await image.decode();
  return image;
}

/** The card's pixels averaged down to one RGBA quad per grid cell, row by row. */
function cellColors(image: HTMLImageElement, columns: number, rows: number) {
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d", { willReadFrequently: true });

  canvas.width = columns;
  canvas.height = rows;
  if (!context) {
    throw new Error("2D canvas is unavailable");
  }
  context.drawImage(image, 0, 0, columns, rows);
  return context.getImageData(0, 0, columns, rows).data;
}

type Box = { x: number; y: number; width: number; height: number };

/** Whether the card point `x, y` lies on an edge of `box`, within most of a cell. */
function onEdge(box: Box, x: number, y: number, cell: { width: number; height: number }) {
  const inside =
    x >= box.x - cell.width &&
    x <= box.x + box.width + cell.width &&
    y >= box.y - cell.height &&
    y <= box.y + box.height + cell.height;
  const nearX = Math.min(Math.abs(x - box.x), Math.abs(x - box.x - box.width)) < cell.width * 0.6;
  const nearY = Math.min(Math.abs(y - box.y), Math.abs(y - box.y - box.height)) < cell.height * 0.6;

  return inside && (nearX || nearY);
}

export async function createParticleScene(
  canvas: HTMLCanvasElement,
  data: StageData,
  options: { columns: number; palette: Palette },
): Promise<ParticleScene> {
  const image = await loadImage(data.image);
  const renderer = new WebGLRenderer({ canvas, antialias: false, alpha: true });
  const scene = new Scene();
  const camera = new PerspectiveCamera(FOV, 1, 1, 20000);

  renderer.outputColorSpace = SRGBColorSpace;
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));

  // ─── Cells, column by column: each takes its layout shade, paint layer, and timing ───

  const columns = options.columns;
  const rows = Math.round((columns * data.height) / data.width);
  const count = columns * rows;
  const colors = cellColors(image, columns, rows);
  const cellSize = { width: data.width / columns, height: data.height / rows };
  const frames: Box[] = [{ x: 0, y: 0, width: data.width, height: data.height }, ...data.boxes];
  const ground = [colors[0] ?? 0, colors[1] ?? 0, colors[2] ?? 0];
  const sources = new Float32Array(count * 2);
  const cells = new Float32Array(count * 2);
  const shades = new Float32Array(count);
  const layers = new Float32Array(count);
  const seeds = new Float32Array(count);
  const delays = new Float32Array(count);

  for (let index = 0; index < count; index++) {
    const column = Math.floor(index / rows);
    const row = index % rows;
    const x = (column + 0.5) * cellSize.width;
    const y = (row + 0.5) * cellSize.height;
    const depth = data.boxes.reduce(
      (deepest, box) =>
        x >= box.x && x <= box.x + box.width && y >= box.y && y <= box.y + box.height
          ? Math.max(deepest, box.depth)
          : deepest,
      0,
    );
    const offset = (row * columns + column) * 4;
    const difference =
      Math.abs((colors[offset] ?? 0) - (ground[0] ?? 0)) +
      Math.abs((colors[offset + 1] ?? 0) - (ground[1] ?? 0)) +
      Math.abs((colors[offset + 2] ?? 0) - (ground[2] ?? 0));
    const seed = Math.abs(Math.sin(index * 12.9898 + 78.233) * 43758.5453) % 1;

    cells[index * 2] = column;
    cells[index * 2 + 1] = row;
    shades[index] = frames.some((box) => onEdge(box, x, y, cellSize)) ? 0.62 : 0.05 + 0.07 * depth;
    layers[index] = difference > 36 ? depth + 1 : 0;
    seeds[index] = seed;
    delays[index] = (column / columns) * 0.85 + seed * 0.15;
  }

  const quad = new PlaneGeometry(1, 1);
  const geometry = new InstancedBufferGeometry();
  const source = new InstancedBufferAttribute(sources, 2);

  geometry.setIndex(quad.getIndex());
  geometry.setAttribute("position", quad.getAttribute("position"));
  geometry.setAttribute("source", source);
  geometry.setAttribute("cell", new InstancedBufferAttribute(cells, 2));
  geometry.setAttribute("shade", new InstancedBufferAttribute(shades, 1));
  geometry.setAttribute("layer", new InstancedBufferAttribute(layers, 1));
  geometry.setAttribute("seed", new InstancedBufferAttribute(seeds, 1));
  geometry.setAttribute("delay", new InstancedBufferAttribute(delays, 1));
  geometry.instanceCount = count;
  quad.dispose();

  const map = new CanvasTexture(image);

  map.colorSpace = SRGBColorSpace;

  const uniforms = {
    progress: { value: 0 },
    stage: { value: new Vector2(1, 1) },
    cardCenter: { value: new Vector2() },
    cardSize: { value: new Vector2(data.width, data.height) },
    grid: { value: new Vector2(columns, rows) },
    tilt: { value: new Matrix3() },
    map: { value: map },
    lod: { value: Math.log2(image.naturalWidth / columns) },
    foreground: { value: new Color() },
    background: { value: new Color() },
  };
  const material = new ShaderMaterial({
    glslVersion: GLSL3,
    vertexShader: VERTEX,
    fragmentShader: FRAGMENT,
    uniforms,
    depthTest: false,
    depthWrite: false,
  });
  const particles = new Mesh(geometry, material);

  particles.frustumCulled = false;
  scene.add(particles);

  const turn = new Matrix4();
  const euler = new Euler();

  const render = (progress: number) => {
    const tilt = ramp(progress, STAGE.tiltIn) * (1 - ramp(progress, STAGE.tiltOut));

    euler.set(-0.95 * tilt, 0, -0.32 * tilt);
    turn.makeRotationFromEuler(euler).multiplyScalar(1 - 0.22 * tilt);
    uniforms.tilt.value.setFromMatrix4(turn);
    uniforms.progress.value = progress;
    renderer.render(scene, camera);
  };

  const layout = ({ width, height, ink, card }: StageLayout) => {
    const points = ink.length / 2;

    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.position.set(0, 0, height / 2 / Math.tan((FOV / 2) * (Math.PI / 180)));
    camera.updateProjectionMatrix();
    uniforms.stage.value.set(width, height);
    uniforms.cardCenter.value.set(
      card.left + card.width / 2 - width / 2,
      height / 2 - card.top - card.height / 2,
    );
    uniforms.cardSize.value.set(card.width, card.height);

    // Cells and points both run left to right, so the headline's left lands on the card's left.
    for (let index = 0; index < count; index++) {
      const point = Math.floor((index * points) / count);

      sources[index * 2] = ink[point * 2] ?? 0;
      sources[index * 2 + 1] = ink[point * 2 + 1] ?? 0;
    }
    source.needsUpdate = true;
  };

  const setPalette = (palette: Palette) => {
    uniforms.foreground.value.set(palette.foreground);
    uniforms.background.value.set(palette.background);
  };

  setPalette(options.palette);
  await renderer.compileAsync(scene, camera);

  return {
    render,
    layout,
    setPalette,
    dispose() {
      geometry.dispose();
      material.dispose();
      map.dispose();
      renderer.dispose();
    },
  };
}
