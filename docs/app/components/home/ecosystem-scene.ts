/**
 * The homepage's one WebGL scene: a single card that scroll progress carries through Takumi,
 * from JSX to layout, paint, pixels, outputs, and runtimes. World units are CSS pixels on the
 * z = 0 plane, so the scene lines up with the DOM poster it replaces.
 */
import {
  BoxGeometry,
  BufferGeometry,
  CanvasTexture,
  Color,
  EllipseCurve,
  GLSL3,
  Group,
  InstancedBufferGeometry,
  LineBasicMaterial,
  LineDashedMaterial,
  LineLoop,
  type Material,
  Mesh,
  MeshBasicMaterial,
  type Object3D,
  PerspectiveCamera,
  PlaneGeometry,
  Scene,
  ShaderMaterial,
  Sprite,
  SpriteMaterial,
  SRGBColorSpace,
  type Texture,
  Vector2,
  WebGLRenderer,
} from "three";
import {
  CARD_LAYOUT,
  cardFrame,
  type EcosystemData,
  type Palette,
  WIDE_MIN,
} from "./ecosystem-data";

const W = CARD_LAYOUT.width;
const H = CARD_LAYOUT.height;
const FOV = 30;
const LAYER_GAP = 120;
const MONO = '"Geist Mono", ui-monospace, monospace';

type Tone = "foreground" | "muted" | "accent";
type Tintable = Material & { color: Color };

/** 0 before `from`, 1 after `to`, eased in and out between. */
function span(progress: number, from: number, to: number) {
  const t = Math.min(Math.max((progress - from) / (to - from), 0), 1);

  return t * t * (3 - 2 * t);
}

function mix(a: number, b: number, t: number) {
  return a + (b - a) * t;
}

/** A repeatable pseudo-random number in -1..1 for `seed`. */
function noise(seed: number) {
  const x = Math.sin(seed * 127.1 + 311.7) * 43758.5453;

  return (x - Math.floor(x)) * 2 - 1;
}

/** A card-space rectangle (x right, y down, origin top-left) in scene space (y up, centered). */
function rectGeometry(x: number, y: number, width: number, height: number) {
  const left = x - W / 2;
  const top = H / 2 - y;

  return new BufferGeometry().setFromPoints([
    new Vector2(left, top),
    new Vector2(left + width, top),
    new Vector2(left + width, top - height),
    new Vector2(left, top - height),
  ]);
}

const VOXEL_VERTEX = `
uniform sampler2D map;
uniform vec2 grid;
uniform float lod;
uniform float relief;
uniform float scatter;
out vec3 color;
out vec3 surface;
void main() {
  float id = float(gl_InstanceID);
  vec2 cell = vec2(mod(id, grid.x), floor(id / grid.x));
  // Rows count down from the top; the texture's v counts up from the bottom.
  vec2 uv = (cell + 0.5) / grid;
  color = textureLod(map, vec2(uv.x, 1.0 - uv.y), lod).rgb;
  float luminance = dot(color, vec3(0.2126, 0.7152, 0.0722));
  vec2 size = vec2(${W}.0, ${H}.0) / grid;
  vec2 center = (cell + 0.5) * size - vec2(${W / 2}.0, ${H / 2}.0);
  center.y = -center.y;
  float seed = fract(sin(id * 12.9898) * 43758.5453);
  float height = 2.0 + luminance * 90.0 * relief;
  vec3 local = vec3(position.xy * size * 0.86, (position.z + 0.5) * height);
  vec3 offset = vec3(0.0, 0.0, (seed - 0.5) * 900.0 * scatter);
  surface = normal;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(vec3(center, 0.0) + local + offset, 1.0);
}`;

const VOXEL_FRAGMENT = `
uniform float opacity;
in vec3 color;
in vec3 surface;
out vec4 fragment;
void main() {
  float light = 0.62 + 0.38 * max(dot(surface, normalize(vec3(-0.3, 0.45, 0.85))), 0.0);
  fragment = vec4(pow(color * light, vec3(1.0 / 2.2)), opacity);
}`;

export type EcosystemScene = {
  /** Draws the scene at `progress`; returns whether ambient motion wants another frame. */
  render: (progress: number, time: number, pointer: { x: number; y: number }) => boolean;
  resize: (width: number, height: number) => void;
  setPalette: (palette: Palette) => void;
  dispose: () => void;
};

async function loadImage(src: string) {
  const image = new Image();

  image.src = src;
  await image.decode();
  return image;
}

function canvasTexture(source: CanvasImageSource, width = W, height = H) {
  const canvas = document.createElement("canvas");

  canvas.width = width;
  canvas.height = height;
  canvas.getContext("2d")?.drawImage(source, 0, 0, width, height);

  const texture = new CanvasTexture(canvas);

  texture.colorSpace = SRGBColorSpace;
  return texture;
}

/** `svg`, rasterized at the card's size. */
async function svgTexture(svg: SVGSVGElement) {
  const copy = svg.cloneNode(true);

  if (!(copy instanceof SVGSVGElement)) {
    throw new Error("Expected an <svg> element");
  }
  copy.setAttribute("width", String(W));
  copy.setAttribute("height", String(H));

  const url = URL.createObjectURL(
    new Blob([new XMLSerializer().serializeToString(copy)], { type: "image/svg+xml" }),
  );

  try {
    return canvasTexture(await loadImage(url));
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** The animated card's frames at a third of its size; none where `ImageDecoder` is missing. */
async function decodeFrames(src: string) {
  if (!("ImageDecoder" in window)) {
    return [];
  }

  const response = await fetch(src);

  if (!response.body) {
    return [];
  }

  const decoder = new ImageDecoder({ data: response.body, type: "image/webp" });

  await decoder.tracks.ready;

  const count = decoder.tracks.selectedTrack?.frameCount ?? 0;
  const frames: HTMLCanvasElement[] = [];

  for (let index = 0; index < count; index++) {
    const { image } = await decoder.decode({ frameIndex: index });
    const canvas = document.createElement("canvas");

    canvas.width = W / 3;
    canvas.height = H / 3;
    canvas.getContext("2d")?.drawImage(image, 0, 0, canvas.width, canvas.height);
    image.close();
    frames.push(canvas);
  }
  decoder.close();
  return frames;
}

/** Lets the browser handle input between the heavier setup phases. */
function yieldToMain() {
  return new Promise<void>((resolve) => setTimeout(resolve, 0));
}

export async function createEcosystemScene(
  canvas: HTMLCanvasElement,
  data: EcosystemData,
  layerSvgs: SVGSVGElement[],
  palette: Palette,
  reduceMotion: boolean,
): Promise<EcosystemScene> {
  await document.fonts.load(`32px ${MONO}`);

  const [cardImage, svgImage, ...layerTextures] = await Promise.all([
    loadImage(data.card.image),
    loadImage(data.card.svg),
    ...layerSvgs.map(svgTexture),
  ]);
  let narrow = canvas.clientWidth < WIDE_MIN;
  const renderer = new WebGLRenderer({ canvas, antialias: true, alpha: true });
  const scene = new Scene();
  const camera = new PerspectiveCamera(FOV, 1, 1, 20000);
  const root = new Group();
  const card = new Group();
  const outputs = new Group();
  const runtimes = new Group();
  const tones = { foreground: new Color(), muted: new Color(), accent: new Color() };
  const tinted: { material: Tintable; tone: Tone }[] = [];
  const screenLabels: { sprite: Sprite; px: number }[] = [];

  renderer.outputColorSpace = SRGBColorSpace;
  renderer.setPixelRatio(Math.min(devicePixelRatio, narrow ? 1.5 : 2));
  scene.add(root);
  root.add(card, outputs, runtimes);

  const cardTexture = new CanvasTexture(cardImage);

  cardTexture.colorSpace = SRGBColorSpace;

  // ─── Materials and labels, tinted from the page's theme ───

  const tint = <M extends Tintable>(material: M, tone: Tone = "foreground") => {
    tinted.push({ material, tone });
    material.userData.base = material.opacity;
    return material;
  };
  const line = (opacity = 1) =>
    tint(new LineBasicMaterial({ transparent: true, opacity, depthWrite: false }));
  const outlineOf = (geometry: BufferGeometry, opacity = 1) =>
    new LineLoop(geometry, line(opacity));
  const cardPlane = (texture: Texture) => {
    const material = new MeshBasicMaterial({ map: texture, transparent: true, depthWrite: false });

    material.userData.base = 1;
    return new Mesh(new PlaneGeometry(W, H), material);
  };

  /**
   * Mono text as a sprite, `px` tall: in screen pixels when `screen` is set, so it stays legible
   * at any card size; otherwise in card units.
   */
  const text = (value: string, px: number, tone: Tone = "foreground", screen = true) => {
    const density = 3;
    const font = `${px * density}px ${MONO}`;
    const surface = document.createElement("canvas");
    const context = surface.getContext("2d");

    if (!context) {
      throw new Error("2D canvas is unavailable");
    }
    context.font = font;
    surface.width = Math.max(1, Math.ceil(context.measureText(value).width));
    surface.height = Math.ceil(px * density * 1.4);
    context.font = font;
    context.fillStyle = "white";
    context.textBaseline = "middle";
    context.fillText(value, 0, surface.height / 2);

    const sprite = new Sprite(
      tint(
        new SpriteMaterial({
          map: new CanvasTexture(surface),
          transparent: true,
          depthWrite: false,
          depthTest: false,
        }),
        tone,
      ),
    );
    const height = px * 1.4;

    sprite.center.set(0, 0.5);
    sprite.scale.set((height * surface.width) / surface.height, height, 1);
    if (screen) {
      screenLabels.push({ sprite, px: height });
    }
    return sprite;
  };

  const setOpacity = (object: Object3D, opacity: number) => {
    object.visible = opacity > 0.002;
    object.traverse((child) => {
      if (child instanceof Mesh || child instanceof LineLoop || child instanceof Sprite) {
        const material = child.material;

        if (!Array.isArray(material)) {
          material.opacity = opacity * (material.userData.base ?? 1);
        }
      }
    });
  };

  // ─── The card outline ───

  const outline = outlineOf(rectGeometry(0, 0, W, H));

  card.add(outline);

  // ─── JSX: source lines drift in depth, then settle on the card ───

  await yieldToMain();

  const code = new Group();
  const lineHeight = H / (data.source.length + 3);
  const codeLines = data.source.map((value, index) => {
    const sprite = text(value || " ", lineHeight * 0.6, "foreground", false);
    const home = { x: -W / 2 + 48, y: H / 2 - (index + 2) * lineHeight };
    const away = {
      x: home.x + noise(index) * 220,
      y: home.y + noise(index + 40) * 140,
      z: -300 + noise(index + 80) * 700,
    };

    code.add(sprite);
    return { sprite, home, away };
  });

  card.add(code);

  // ─── Layout: the real boxes, lifted by their depth in the tree ───

  const layout = new Group();
  const boxes = data.boxes.map((box) => {
    const group = new Group();
    const frame = new LineLoop(
      rectGeometry(box.x, box.y, box.width, box.height),
      box.leaf
        ? line(0.9)
        : tint(
            new LineDashedMaterial({ dashSize: 10, gapSize: 8, transparent: true, opacity: 0.5 }),
          ),
    );

    frame.computeLineDistances();
    group.add(frame);
    if (box.leaf) {
      const tag = text(box.label, 11, "muted");

      tag.position.set(box.x - W / 2, H / 2 - box.y - box.height - 18, 0);
      group.add(tag);
    }
    layout.add(group);
    return { group, depth: box.depth };
  });

  card.add(layout);

  // ─── Paint: the draw steps as layers ───

  const paint = new Group();
  const layers = layerTextures.map((texture, index) => {
    const group = new Group();
    const tag = text(data.steps[index] ?? "", 11, "muted");

    tag.position.set(W / 2 + 32, -H / 2 + 24, 0);
    group.add(cardPlane(texture), outlineOf(rectGeometry(0, 0, W, H), 0.3), tag);
    paint.add(group);
    return { group, tag };
  });

  card.add(paint);

  // ─── Pixels: the render as an instanced field of blocks ───

  const columns = narrow ? 120 : 200;
  const rows = Math.round((columns * H) / W);
  const block = new BoxGeometry(1, 1, 1);
  const voxelGeometry = new InstancedBufferGeometry();

  voxelGeometry.setIndex(block.getIndex());
  voxelGeometry.setAttribute("position", block.getAttribute("position"));
  voxelGeometry.setAttribute("normal", block.getAttribute("normal"));
  const uniforms = {
    map: { value: cardTexture },
    grid: { value: new Vector2(columns, rows) },
    lod: { value: Math.log2(cardImage.naturalWidth / columns) },
    relief: { value: 0 },
    scatter: { value: 1 },
    opacity: { value: 0 },
  };

  voxelGeometry.instanceCount = columns * rows;

  const voxels = new Mesh(
    voxelGeometry,
    new ShaderMaterial({
      glslVersion: GLSL3,
      vertexShader: VOXEL_VERTEX,
      fragmentShader: VOXEL_FRAGMENT,
      transparent: true,
      uniforms,
    }),
  );
  const image = cardPlane(cardTexture);

  voxels.frustumCulled = false;
  card.add(voxels, image);

  await yieldToMain();

  // ─── Outputs: the finished card fans out into every backend ───

  const flipCanvas = document.createElement("canvas");

  flipCanvas.width = W / 3;
  flipCanvas.height = H / 3;
  flipCanvas.getContext("2d")?.drawImage(cardImage, 0, 0, flipCanvas.width, flipCanvas.height);

  const flipTexture = new CanvasTexture(flipCanvas);
  const vectorTexture = canvasTexture(svgImage);
  const frames: HTMLCanvasElement[] = [];

  flipTexture.colorSpace = SRGBColorSpace;
  void decodeFrames(data.card.motion).then((decoded) => frames.push(...decoded));

  const stacked = (depth: number, object: Object3D) => {
    object.position.set(depth * 36, depth * 36, -depth * 60);
    return object;
  };
  const pageWidth = H * 0.95;
  const pageHeight = pageWidth * (842 / 595);
  const pageContent = cardPlane(cardTexture);

  pageContent.scale.setScalar((pageWidth * 0.84) / W);
  pageContent.position.y = pageHeight / 2 - pageWidth * 0.08 - (pageWidth * 0.84 * H) / W / 2;

  const panelContents: Object3D[][] = [
    [cardPlane(cardTexture), outlineOf(rectGeometry(0, 0, W, H), 0.35)],
    [
      stacked(2, outlineOf(rectGeometry(0, 0, W, H), 0.3)),
      stacked(1, outlineOf(rectGeometry(0, 0, W, H), 0.3)),
      cardPlane(flipTexture),
    ],
    [cardPlane(vectorTexture), outlineOf(rectGeometry(0, 0, W, H), 0.35)],
    [
      ...[2, 1, 0].map((depth) =>
        stacked(
          depth,
          outlineOf(
            rectGeometry(W / 2 - pageWidth / 2, H / 2 - pageHeight / 2, pageWidth, pageHeight),
            depth === 0 ? 0.8 : 0.3,
          ),
        ),
      ),
      pageContent,
    ],
    [
      (() => {
        const stack = new Group();

        stack.rotation.set(-0.95, 0, -0.5);
        stack.scale.setScalar(0.8);
        layerTextures.forEach((texture, index) => {
          const plane = cardPlane(texture);
          const frame = outlineOf(rectGeometry(0, 0, W, H), 0.3);

          plane.position.z = (index - (layerTextures.length - 1) / 2) * LAYER_GAP;
          frame.position.z = plane.position.z;
          stack.add(plane, frame);
        });
        return stack;
      })(),
    ],
  ];
  const panels = panelContents.map((contents, index) => {
    const body = new Group();
    const title = text(data.outputs[index]?.title ?? "", 11);
    const detail = text(data.outputs[index]?.detail ?? "", 11, "muted");

    body.add(...contents);
    outputs.add(body, title, detail);
    return { body, title, detail };
  });

  // ─── Runtimes: everything folds into one core, with the runtimes around it ───

  const core = outlineOf(rectGeometry(W / 2 - 110, H / 2 - 110, 220, 220));
  const coreDot = new Mesh(
    new PlaneGeometry(36, 36),
    tint(new MeshBasicMaterial({ transparent: true, depthWrite: false }), "accent"),
  );
  const rings = data.runtimes.map((runtime, index) => {
    const radius = 360 + index * 150;
    const ring = new LineLoop(
      new BufferGeometry().setFromPoints(
        new EllipseCurve(0, 0, radius, radius * 0.36, 0, Math.PI * 2).getPoints(96),
      ),
      line(0.4),
    );
    const tag = text(`${runtime.name}  ${runtime.detail}`, 12);

    tag.center.set(0.5, -0.4);
    ring.rotation.z = -0.22 + index * 0.16;
    runtimes.add(ring, tag);
    return { ring, tag, radius, phase: 0.6 + index * 2.1 };
  });

  runtimes.add(core, coreDot);

  // ─── Layout and drawing ───

  let slots: { x: number; y: number; scale: number }[] = [];
  let lastFlip = 0;
  let flipIndex = 0;

  const resize = (width: number, height: number) => {
    narrow = width < WIDE_MIN;
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.position.set(0, 0, height / 2 / Math.tan((FOV / 2) * (Math.PI / 180)));
    camera.updateProjectionMatrix();

    const frame = cardFrame(width, height);
    const scale = frame.width / W;
    const panelScale = narrow ? 0.46 : 0.34;
    const across = narrow ? 2 : 3;
    const count = data.outputs.length;
    const rowCount = Math.ceil(count / across);

    root.position.set(frame.x, frame.y, 0);
    root.scale.setScalar(scale);
    for (const { sprite, px } of screenLabels) {
      const aspect = sprite.scale.x / sprite.scale.y;

      sprite.scale.set((px * aspect) / scale, px / scale, 1);
    }
    slots = Array.from({ length: count }, (_, index) => {
      const row = Math.floor(index / across);
      const inRow = Math.min(across, count - row * across);
      const column = index - row * across;

      return {
        x: (column - (inRow - 1) / 2) * W * panelScale * 1.32,
        y: ((rowCount - 1) / 2 - row) * H * panelScale * (narrow ? 2.25 : 2.1),
        scale: panelScale,
      };
    });
  };

  const setPalette = (next: Palette) => {
    tones.foreground.set(next.foreground);
    tones.muted.set(next.muted);
    tones.accent.set(next.accent);
    for (const { material, tone } of tinted) {
      material.color.copy(tones[tone]);
    }
  };

  const render = (p: number, time: number, pointer: { x: number; y: number }) => {
    const hero = 1 - span(p, 0, 0.5);
    const ambient = reduceMotion ? 0 : hero;
    const tilt = span(p, 1.3, 1.9) * (1 - span(p, 4.15, 4.6));

    card.rotation.set(
      -0.95 * tilt + pointer.y * 0.12 * ambient + Math.sin(time * 0.0006) * 0.015 * ambient,
      pointer.x * 0.18 * ambient,
      -0.5 * tilt,
    );
    card.position.z = Math.sin(time * 0.0008) * 8 * ambient;
    card.scale.setScalar(1 - 0.2 * tilt);

    setOpacity(outline, (0.9 - 0.55 * span(p, 0.4, 1)) * (1 - span(p, 4.5, 4.7)));

    const settle = span(p, 0.3, 1);

    setOpacity(code, span(p, 0.25, 0.55) * (1 - span(p, 1.3, 1.6)));
    for (const { sprite, home, away } of codeLines) {
      sprite.position.set(
        mix(away.x, home.x, settle),
        mix(away.y, home.y, settle),
        mix(away.z, 0, settle),
      );
    }

    const lift = span(p, 1.5, 2);

    setOpacity(layout, span(p, 1.35, 1.7) * (1 - span(p, 2.35, 2.6)));
    for (const { group, depth } of boxes) {
      group.position.z = depth * 70 * lift;
    }

    const explode = span(p, 2.4, 2.95) * (1 - span(p, 3.35, 3.7));

    const painted = span(p, 2.3, 2.55) * (1 - span(p, 3.6, 3.8));

    setOpacity(paint, painted);
    layers.forEach(({ group, tag }, index) => {
      group.position.z = (index - (layers.length - 1) / 2) * LAYER_GAP * explode;
      setOpacity(tag, painted * explode);
    });

    uniforms.opacity.value = span(p, 3.6, 3.85) * (1 - span(p, 4.5, 4.7));
    uniforms.relief.value = span(p, 3.7, 4) * (1 - span(p, 4.15, 4.55));
    uniforms.scatter.value = 1 - span(p, 3.6, 4);
    voxels.visible = uniforms.opacity.value > 0.002;
    setOpacity(image, p < 4.75 ? span(p, 4.45, 4.7) : 0);

    const fan = span(p, 4.72, 4.98);
    const fold = span(p, 5.4, 5.8);

    setOpacity(outputs, p >= 4.75 ? 1 - span(p, 5.55, 5.8) : 0);
    panels.forEach(({ body, title, detail }, index) => {
      const slot = slots[index] ?? { x: 0, y: 0, scale: 1 };
      const scale = mix(mix(1, slot.scale, fan), 0.12, fold);
      const x = mix(slot.x * fan, 0, fold);
      const y = mix(slot.y * fan, 0, fold);
      const gap = 22 / root.scale.x;

      body.position.set(x, y, 0);
      body.scale.setScalar(scale);
      title.position.set(x - (W / 2) * scale, y - (H / 2) * scale - gap, 0);
      detail.position.set(x - (W / 2) * scale, y - (H / 2) * scale - gap * 1.9, 0);
      setOpacity(title, fan * (1 - fold));
      setOpacity(detail, fan * (1 - fold));
    });

    const present = span(p, 5.6, 5.95);

    setOpacity(runtimes, present);
    runtimes.scale.setScalar(mix(0.7, 1, present) * (narrow ? 0.72 : 1));
    for (const { ring, tag, radius, phase } of rings) {
      const angle = phase + (reduceMotion ? 0 : time * 0.00012);
      const x = Math.cos(angle) * radius;
      const y = Math.sin(angle) * radius * 0.36;
      const turn = ring.rotation.z;

      tag.position.set(
        x * Math.cos(turn) - y * Math.sin(turn),
        x * Math.sin(turn) + y * Math.cos(turn),
        0,
      );
    }

    const flipping = p > 4.8 && p < 5.6 && frames.length > 0 && !reduceMotion;

    if (flipping && time - lastFlip > 33) {
      lastFlip = time;
      flipIndex = (flipIndex + 1) % frames.length;

      const frame = frames[flipIndex];

      if (frame) {
        flipCanvas.getContext("2d")?.drawImage(frame, 0, 0);
        flipTexture.needsUpdate = true;
      }
    }

    renderer.render(scene, camera);
    return !reduceMotion && (hero > 0.01 || flipping || (present > 0.01 && p < 6.6));
  };

  setPalette(palette);
  resize(canvas.clientWidth, canvas.clientHeight);
  await yieldToMain();
  await renderer.compileAsync(scene, camera);

  return {
    render,
    resize,
    setPalette,
    dispose() {
      scene.traverse((object) => {
        if (object instanceof Mesh || object instanceof LineLoop || object instanceof Sprite) {
          object.geometry.dispose();

          const material = object.material;

          if (!Array.isArray(material)) {
            if (material instanceof SpriteMaterial) {
              material.map?.dispose();
            }
            material.dispose();
          }
        }
      });
      for (const texture of [cardTexture, flipTexture, vectorTexture, ...layerTextures]) {
        texture.dispose();
      }
      renderer.dispose();
    },
  };
}
