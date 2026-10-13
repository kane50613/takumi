/**
 * Lazy-loaded Three.js forge scene: soft circular forge embers burst outward from the anvil on
 * each hammer strike and drift downward toward the outputs section.
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
import { layoutSpread, paintSpread, strikeImpulse, textSpread } from "./stage-timeline";

const FOV = 32;

export type StageData = {
  image: string;
  width: number;
  height: number;
  boxes: { depth: number; x: number; y: number; width: number; height: number }[];
};

export type Palette = { foreground: string; background: string };

export type StageLayout = {
  width: number;
  height: number;
  card: { left: number; top: number; width: number; height: number };
};

export type ParticleScene = {
  render: (progress: number) => void;
  layout: (layout: StageLayout) => void;
  setPalette: (palette: Palette) => void;
  dispose: () => void;
};

const VERTEX = `
uniform float progress;
uniform float impulse;
uniform float layoutAmt;
uniform float textAmt;
uniform float paintAmt;
uniform vec2 stage;
uniform vec2 cardCenter;
uniform vec2 cardSize;
uniform vec2 grid;
uniform mat3 tilt;
uniform sampler2D map;
uniform float lod;
uniform vec3 emberWarm;
uniform vec3 emberHot;
in vec2 cell;
in float shade;
in float layer;
in float seed;
out vec3 tint;
out float alpha;
out vec2 vUv;

void main() {
  vUv = position.xy + 0.5;
  vec2 uv = (cell + 0.5) / grid;
  vec2 centered = uv - 0.5;
  float angle = seed * 6.2831853 + centered.x * 2.1;
  float radius = (0.28 + fract(seed * 7.13) * 0.65) * max(cardSize.x, cardSize.y) * 0.72;

  float activeSpread = max(layoutAmt, max(textAmt, paintAmt));
  float burst = clamp(activeSpread * 0.85 + impulse * 0.75, 0.0, 1.25);

  vec3 anvilLocal = vec3(
    centered.x * cardSize.x * 0.85,
    -centered.y * cardSize.y * 0.85,
    0.0
  );

  vec3 radialOut = vec3(
    cos(angle) * radius * burst,
    sin(angle) * radius * 0.68 * burst + impulse * 75.0 * (fract(seed * 3.7) - 0.25),
    (layer * 45.0 + fract(seed * 11.3) * 160.0) * burst
  );

  vec3 worldPos = vec3(cardCenter, 0.0) + tilt * (anvilLocal * 0.55 + radialOut);

  float emberSize = mix(4.0, 10.0, fract(seed * 9.2)) * (0.45 + 0.75 * burst);
  vec3 corner = vec3(position.xy * emberSize, 0.0);

  gl_Position = projectionMatrix * modelViewMatrix * vec4(worldPos + corner, 1.0);

  vec3 pixel = textureLod(map, vec2(uv.x, 1.0 - uv.y), lod).rgb;
  vec3 spark = mix(emberHot, emberWarm, fract(seed * 4.3 + impulse * 0.5));
  tint = mix(spark, pixel, clamp(paintAmt * 0.55, 0.0, 1.0));

  float sparse = step(0.62, fract(seed * 17.3 + shade * 0.5));
  alpha = clamp((burst * 0.55 + impulse * 0.75) * sparse, 0.0, 0.68);
}`;

const FRAGMENT = `
in vec3 tint;
in float alpha;
in vec2 vUv;
out vec4 fragment;
void main() {
  float dist = length(vUv - 0.5) * 2.0;
  if (dist > 1.0 || alpha < 0.02) {
    discard;
  }
  float soft = (1.0 - dist * dist) * alpha;
  fragment = linearToOutputTexel(vec4(tint, soft));
}`;

async function loadImage(src: string) {
  const image = new Image();

  image.src = src;
  await image.decode();
  return image;
}

type Box = { x: number; y: number; width: number; height: number };

function onEdge(box: Box, x: number, y: number, cell: { width: number; height: number }) {
  const inside =
    x >= box.x - cell.width &&
    x <= box.x + box.width + cell.width &&
    y >= box.y - cell.height &&
    y <= box.y + box.height + cell.height;
  const nearX = Math.min(Math.abs(x - box.x), Math.abs(x - box.x - box.width)) < cell.width * 0.8;
  const nearY = Math.min(Math.abs(y - box.y), Math.abs(y - box.y - box.height)) < cell.height * 0.8;

  return inside && (nearX || nearY);
}

export async function createParticleScene(
  canvas: HTMLCanvasElement,
  data: StageData,
  options: { columns: number; palette: Palette },
): Promise<ParticleScene> {
  const image = await loadImage(data.image);
  const renderer = new WebGLRenderer({ canvas, antialias: true, alpha: true });
  const scene = new Scene();
  const camera = new PerspectiveCamera(FOV, 1, 1, 20000);

  renderer.outputColorSpace = SRGBColorSpace;
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));

  const columns = Math.min(options.columns, 28);
  const rows = Math.max(10, Math.round((columns * data.height) / data.width));
  const count = columns * rows;
  const cellSize = { width: data.width / columns, height: data.height / rows };
  const frames: Box[] = [{ x: 0, y: 0, width: data.width, height: data.height }, ...data.boxes];
  const cells = new Float32Array(count * 2);
  const shades = new Float32Array(count);
  const layers = new Float32Array(count);
  const seeds = new Float32Array(count);

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
    const seed = Math.abs(Math.sin(index * 12.9898 + 78.233) * 43758.5453) % 1;

    cells[index * 2] = column;
    cells[index * 2 + 1] = row;
    shades[index] = frames.some((box) => onEdge(box, x, y, cellSize)) ? 0.9 : 0.15 + 0.08 * depth;
    layers[index] = depth + 1;
    seeds[index] = seed;
  }

  const quad = new PlaneGeometry(1, 1);
  const geometry = new InstancedBufferGeometry();

  geometry.setIndex(quad.getIndex());
  geometry.setAttribute("position", quad.getAttribute("position"));
  geometry.setAttribute("cell", new InstancedBufferAttribute(cells, 2));
  geometry.setAttribute("shade", new InstancedBufferAttribute(shades, 1));
  geometry.setAttribute("layer", new InstancedBufferAttribute(layers, 1));
  geometry.setAttribute("seed", new InstancedBufferAttribute(seeds, 1));
  geometry.instanceCount = count;
  quad.dispose();

  const map = new CanvasTexture(image);

  map.colorSpace = SRGBColorSpace;

  const uniforms = {
    progress: { value: 0 },
    impulse: { value: 0 },
    layoutAmt: { value: 0 },
    textAmt: { value: 0 },
    paintAmt: { value: 0 },
    stage: { value: new Vector2(1, 1) },
    cardCenter: { value: new Vector2() },
    cardSize: { value: new Vector2(data.width, data.height) },
    grid: { value: new Vector2(columns, rows) },
    tilt: { value: new Matrix3() },
    map: { value: map },
    lod: { value: Math.max(0, Math.log2(image.naturalWidth / columns)) },
    emberWarm: { value: new Color("#ffa944") },
    emberHot: { value: new Color("#ff3535") },
  };
  const material = new ShaderMaterial({
    glslVersion: GLSL3,
    vertexShader: VERTEX,
    fragmentShader: FRAGMENT,
    uniforms,
    transparent: true,
    depthTest: false,
    depthWrite: false,
  });
  const particles = new Mesh(geometry, material);

  particles.frustumCulled = false;
  scene.add(particles);

  const turn = new Matrix4();
  const euler = new Euler();

  const render = (progress: number) => {
    const lSpread = layoutSpread(progress);
    const tSpread = textSpread(progress);
    const pSpread = paintSpread(progress);
    const imp = strikeImpulse(progress);
    const tiltAmount = Math.max(lSpread * 0.35, pSpread * 0.55);

    euler.set(-0.35 * tiltAmount, 0.14 * pSpread, -0.12 * tiltAmount);
    turn.makeRotationFromEuler(euler);
    uniforms.tilt.value.setFromMatrix4(turn);
    uniforms.progress.value = progress;
    uniforms.impulse.value = imp;
    uniforms.layoutAmt.value = lSpread;
    uniforms.textAmt.value = tSpread;
    uniforms.paintAmt.value = pSpread;
    renderer.render(scene, camera);
  };

  const layout = ({ width, height, card }: StageLayout) => {
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
  };

  const setPalette = (_palette: Palette) => {
    uniforms.emberWarm.value.set("#ffa944");
    uniforms.emberHot.value.set("#ff3535");
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
