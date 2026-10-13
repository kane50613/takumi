/**
 * The homepage's one WebGL moment: every character of the card's source sheds particles that fly
 * onto the card, shade into its layout boxes, lift apart as paint layers, and settle as its
 * pixels. World units are the stage's CSS pixels on the z = 0 plane, centered, with y up.
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
const TILE = 64;
const ATLAS_COLUMNS = 16;
const LAYER_GAP = 150;

export type StageData = {
  /** Lines of the card's JSX source. */
  source: string[];
  image: string;
  width: number;
  height: number;
  boxes: { depth: number; x: number; y: number; width: number; height: number }[];
};

export type Palette = { foreground: string; background: string };

/** The source text's first character cell and the card's box, in CSS pixels from the stage's top left. */
export type StageLayout = {
  width: number;
  height: number;
  source: { left: number; top: number; charWidth: number; lineHeight: number };
  card: { left: number; top: number; width: number; height: number };
};

export type ParticleScene = {
  render: (progress: number) => void;
  layout: (layout: StageLayout) => void;
  setPalette: (palette: Palette) => void;
  dispose: () => void;
};

const easing = (name: string, [from, to]: readonly [number, number]) =>
  `float ${name}() { return smoothstep(0.0, 1.0, clamp((progress - ${from.toFixed(3)}) / ${(to - from).toFixed(3)}, 0.0, 1.0)); }`;

const VERTEX = `
uniform float progress;
uniform vec2 sourceOrigin;
uniform vec2 sourceCell;
uniform vec2 cardCenter;
uniform vec2 cardSize;
uniform vec2 grid;
uniform mat3 tilt;
uniform sampler2D map;
uniform float lod;
uniform vec2 atlasGrid;
uniform vec3 foreground;
uniform vec3 background;
in vec2 source;
in vec2 cell;
in float glyph;
in float shade;
in float layer;
in float seed;
in float delay;
in float leader;
out vec2 atlasUv;
out vec3 tint;
out float morph;
out float visible;
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
  vec3 from = vec3(sourceOrigin + vec2(source.x + 0.5, -(source.y + 0.5)) * sourceCell, 0.0);
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
  vec3 position3 = mix(mix(from, control, t), mix(control, landed, t), t);
  morph = smoothstep(0.0, 0.55, fly);
  vec2 size = mix(vec2(sourceCell.y), cellSize * mix(0.78, 1.02, fill()), morph);
  vec3 corner = vec3(position.xy * size, 0.0);
  corner = mix(corner, tilt * corner, morph);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position3 + corner, 1.0);

  vec2 cellUv = (cell + 0.5) / grid;
  vec3 pixel = textureLod(map, vec2(cellUv.x, 1.0 - cellUv.y), lod).rgb;
  vec3 boxes = mix(background, foreground, shade);
  tint = mix(foreground, boxes, layoutShade() * morph);
  tint = mix(tint, pixel, paintColor() * morph);
  visible = leader > 0.5 ? 1.0 : smoothstep(0.0, 0.08, fly);
  vec2 tile = vec2(mod(glyph, atlasGrid.x), floor(glyph / atlasGrid.x));
  atlasUv = vec2((tile.x + uv.x) / atlasGrid.x, 1.0 - (tile.y + 1.0 - uv.y) / atlasGrid.y);
}`;

const FRAGMENT = `
uniform sampler2D atlas;
in vec2 atlasUv;
in vec3 tint;
in float morph;
in float visible;
out vec4 fragment;
void main() {
  float alpha = mix(texture(atlas, atlasUv).a, 1.0, morph) * visible;
  if (alpha < 0.01) {
    discard;
  }
  fragment = linearToOutputTexel(vec4(tint, alpha));
}`;

async function loadImage(src: string) {
  const image = new Image();

  image.src = src;
  await image.decode();
  return image;
}

/** Every distinct non-space character of `lines`, drawn white into a grid of square tiles. */
function glyphAtlas(characters: string[], fontRatio: number, fontFamily: string) {
  const rows = Math.ceil(characters.length / ATLAS_COLUMNS);
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d");

  canvas.width = ATLAS_COLUMNS * TILE;
  canvas.height = rows * TILE;
  if (!context) {
    throw new Error("2D canvas is unavailable");
  }
  context.font = `${TILE * fontRatio}px ${fontFamily}`;
  context.fillStyle = "white";
  context.textAlign = "center";
  context.textBaseline = "middle";
  characters.forEach((character, index) => {
    const x = (index % ATLAS_COLUMNS) * TILE + TILE / 2;
    const y = Math.floor(index / ATLAS_COLUMNS) * TILE + TILE / 2;

    context.fillText(character, x, y);
  });

  return { texture: new CanvasTexture(canvas), size: new Vector2(ATLAS_COLUMNS, rows) };
}

/** The card's pixels averaged down to one RGB triple per grid cell. */
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

export async function createParticleScene(
  canvas: HTMLCanvasElement,
  data: StageData,
  options: { columns: number; fontRatio: number; fontFamily: string; palette: Palette },
): Promise<ParticleScene> {
  const image = await loadImage(data.image);
  const renderer = new WebGLRenderer({ canvas, antialias: false, alpha: true });
  const scene = new Scene();
  const camera = new PerspectiveCamera(FOV, 1, 1, 20000);

  renderer.outputColorSpace = SRGBColorSpace;
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));

  // ─── Characters, in reading order ───

  const characters: { column: number; row: number; glyph: number }[] = [];
  const glyphs = new Map<string, number>();

  data.source.forEach((line, row) => {
    [...line].forEach((character, column) => {
      if (character.trim() === "") {
        return;
      }

      let glyph = glyphs.get(character);

      if (glyph === undefined) {
        glyph = glyphs.size;
        glyphs.set(character, glyph);
      }
      characters.push({ column, row, glyph });
    });
  });

  const atlas = glyphAtlas([...glyphs.keys()], options.fontRatio, options.fontFamily);

  // ─── Cells: each takes a character in reading order, its layout box, and its paint layer ───

  const columns = options.columns;
  const rows = Math.round((columns * data.height) / data.width);
  const count = columns * rows;
  const colors = cellColors(image, columns, rows);
  const cellWidth = data.width / columns;
  const cellHeight = data.height / rows;
  const ground = [colors[0] ?? 0, colors[1] ?? 0, colors[2] ?? 0];
  const sources = new Float32Array(count * 2);
  const cells = new Float32Array(count * 2);
  const glyphIndex = new Float32Array(count);
  const shades = new Float32Array(count);
  const layers = new Float32Array(count);
  const seeds = new Float32Array(count);
  const delays = new Float32Array(count);
  const leaders = new Float32Array(count);
  let previous = -1;

  for (let index = 0; index < count; index++) {
    const column = index % columns;
    const row = Math.floor(index / columns);
    const characterIndex = Math.floor((index * characters.length) / count);
    const character = characters[characterIndex];
    const x = (column + 0.5) * cellWidth;
    const y = (row + 0.5) * cellHeight;
    const depth = data.boxes.reduce(
      (deepest, box) =>
        x >= box.x && x <= box.x + box.width && y >= box.y && y <= box.y + box.height
          ? Math.max(deepest, box.depth)
          : deepest,
      0,
    );
    const onEdge = [
      { depth: 0, x: 0, y: 0, width: data.width, height: data.height },
      ...data.boxes,
    ].some((box) => {
      const inside =
        x >= box.x - cellWidth &&
        x <= box.x + box.width + cellWidth &&
        y >= box.y - cellHeight &&
        y <= box.y + box.height + cellHeight;
      const nearX =
        Math.min(Math.abs(x - box.x), Math.abs(x - box.x - box.width)) < cellWidth * 0.6;
      const nearY =
        Math.min(Math.abs(y - box.y), Math.abs(y - box.y - box.height)) < cellHeight * 0.6;

      return inside && (nearX || nearY);
    });
    const offset = index * 4;
    const difference =
      Math.abs((colors[offset] ?? 0) - (ground[0] ?? 0)) +
      Math.abs((colors[offset + 1] ?? 0) - (ground[1] ?? 0)) +
      Math.abs((colors[offset + 2] ?? 0) - (ground[2] ?? 0));
    const seed = Math.abs(Math.sin(index * 12.9898 + 78.233) * 43758.5453) % 1;

    sources[index * 2] = character?.column ?? 0;
    sources[index * 2 + 1] = character?.row ?? 0;
    cells[index * 2] = column;
    cells[index * 2 + 1] = row;
    glyphIndex[index] = character?.glyph ?? 0;
    shades[index] = onEdge ? 0.62 : 0.05 + 0.07 * depth;
    layers[index] = difference > 36 ? depth + 1 : 0;
    seeds[index] = seed;
    delays[index] = (characterIndex / characters.length) * 0.85 + seed * 0.15;
    leaders[index] = characterIndex === previous ? 0 : 1;
    previous = characterIndex;
  }

  const quad = new PlaneGeometry(1, 1);
  const geometry = new InstancedBufferGeometry();

  geometry.setIndex(quad.getIndex());
  geometry.setAttribute("position", quad.getAttribute("position"));
  geometry.setAttribute("uv", quad.getAttribute("uv"));
  geometry.setAttribute("source", new InstancedBufferAttribute(sources, 2));
  geometry.setAttribute("cell", new InstancedBufferAttribute(cells, 2));
  geometry.setAttribute("glyph", new InstancedBufferAttribute(glyphIndex, 1));
  geometry.setAttribute("shade", new InstancedBufferAttribute(shades, 1));
  geometry.setAttribute("layer", new InstancedBufferAttribute(layers, 1));
  geometry.setAttribute("seed", new InstancedBufferAttribute(seeds, 1));
  geometry.setAttribute("delay", new InstancedBufferAttribute(delays, 1));
  geometry.setAttribute("leader", new InstancedBufferAttribute(leaders, 1));
  geometry.instanceCount = count;
  quad.dispose();

  const map = new CanvasTexture(image);

  map.colorSpace = SRGBColorSpace;

  const uniforms = {
    progress: { value: 0 },
    sourceOrigin: { value: new Vector2() },
    sourceCell: { value: new Vector2(1, 1) },
    cardCenter: { value: new Vector2() },
    cardSize: { value: new Vector2(data.width, data.height) },
    grid: { value: new Vector2(columns, rows) },
    tilt: { value: new Matrix3() },
    map: { value: map },
    lod: { value: Math.log2(image.naturalWidth / columns) },
    atlas: { value: atlas.texture },
    atlasGrid: { value: atlas.size },
    foreground: { value: new Color() },
    background: { value: new Color() },
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
    const tilt = ramp(progress, STAGE.tiltIn) * (1 - ramp(progress, STAGE.tiltOut));

    euler.set(-0.95 * tilt, 0, -0.32 * tilt);
    turn.makeRotationFromEuler(euler).multiplyScalar(1 - 0.22 * tilt);
    uniforms.tilt.value.setFromMatrix4(turn);
    uniforms.progress.value = progress;
    renderer.render(scene, camera);
  };

  const layout = ({ width, height, source, card }: StageLayout) => {
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.position.set(0, 0, height / 2 / Math.tan((FOV / 2) * (Math.PI / 180)));
    camera.updateProjectionMatrix();
    uniforms.sourceOrigin.value.set(source.left - width / 2, height / 2 - source.top);
    uniforms.sourceCell.value.set(source.charWidth, source.lineHeight);
    uniforms.cardCenter.value.set(
      card.left + card.width / 2 - width / 2,
      height / 2 - card.top - card.height / 2,
    );
    uniforms.cardSize.value.set(card.width, card.height);
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
      atlas.texture.dispose();
      renderer.dispose();
    },
  };
}
