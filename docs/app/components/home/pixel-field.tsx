"use client";

import { ArrowRight } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type * as ThreeModule from "three";

type Three = typeof ThreeModule;

const RENDERS = [
  { name: "home-demo-card.tsx", src: "/images/hero/demo-card@2x.webp" },
  { name: "dither-og.tsx", src: "/images/twitter-images/dither-og.png" },
  { name: "package-og-image.tsx", src: "/images/twitter-images/package-og-image.png" },
];

const COLUMNS = 192;
const ROWS = 101;
const COUNT = COLUMNS * ROWS;
const PITCH = 0.068;
const RELIEF = 0.55;
const SCAN_MS = 1400;
const DROP_MS = 520;
const POINTER_RADIUS = 1.1;
const WAVE_SPEED = 13;
const WAVE_WIDTH = 0.45;
const WAVE_HEIGHT = 1.4;
const WAVE_DECAY_S = 1.1;
const FLASH_MS = 700;
const WAVE_REACH = Math.hypot(COLUMNS * PITCH, ROWS * PITCH) + 4 * WAVE_WIDTH;
const TILT = { x: -0.42, y: 0.28, z: 0.06 };
const OFF_FIELD = 9e3;
const CAMERA_DISTANCE = 17;

/** `t` in 0..1, eased like `cubic-bezier(0.23, 1, 0.32, 1)`. */
function easeOut(t: number) {
  return 1 - (1 - Math.min(Math.max(t, 0), 1)) ** 4;
}

/** One render, downsampled to a linear RGB color and a block height per block. */
type Relief = { colors: Float32Array; depths: Float32Array };

async function sampleRelief(THREE: Three, src: string): Promise<Relief> {
  const image = new Image();

  image.src = src;
  await image.decode();

  const canvas = document.createElement("canvas");

  canvas.width = COLUMNS;
  canvas.height = ROWS;

  const context = canvas.getContext("2d", { willReadFrequently: true });

  if (!context) {
    throw new Error("2D canvas is unavailable");
  }

  context.imageSmoothingQuality = "high";
  context.drawImage(image, 0, 0, COLUMNS, ROWS);

  const pixels = context.getImageData(0, 0, COLUMNS, ROWS).data;
  const colors = new Float32Array(COUNT * 3);
  const depths = new Float32Array(COUNT);
  const color = new THREE.Color();

  for (let index = 0; index < COUNT; index++) {
    const r = pixels[index * 4] ?? 0;
    const g = pixels[index * 4 + 1] ?? 0;
    const b = pixels[index * 4 + 2] ?? 0;

    color.setRGB(r / 255, g / 255, b / 255, THREE.SRGBColorSpace).toArray(colors, index * 3);
    depths[index] = 0.06 + ((0.2126 * r + 0.7152 * g + 0.0722 * b) / 255) * RELIEF;
  }

  return { colors, depths };
}

/** A ring that lifts blocks as it spreads and leaves `render` behind it. */
type Wave = { x: number; y: number; start: number; order: number; render: number };

type Field = {
  /** Sends a wave from `ndc`, or from the middle of the field, that turns it into the next render. */
  next: (ndc?: { x: number; y: number }) => void;
  dispose: () => void;
};

function createField(
  THREE: Three,
  element: HTMLElement,
  first: Relief,
  onRender: (index: number) => void,
): Field {
  const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const finePointer = matchMedia("(hover: hover) and (pointer: fine)").matches;
  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(30, 1, 0.1, 100);
  const field = new THREE.InstancedMesh(
    new THREE.BoxGeometry(PITCH * 0.92, PITCH * 0.92, 1),
    new THREE.MeshStandardMaterial({ roughness: 0.55, metalness: 0.05 }),
    COUNT,
  );
  const key = new THREE.DirectionalLight(0xffffff, 2.4);
  const lamp = new THREE.PointLight(0xff4d4d, 0, 3.2, 1.6);
  const flash = new THREE.PointLight(0xffd2c2, 0, 6, 1.4);
  const color = new THREE.Color();
  const matrix = new THREE.Matrix4();
  const position = new THREE.Vector3();
  const scale = new THREE.Vector3();
  const rotation = new THREE.Quaternion();
  const reliefs: (Relief | undefined)[] = [first];
  const home = new Float32Array(COUNT * 2);
  const lift = new Float32Array(COUNT);
  const liftVelocity = new Float32Array(COUNT);
  const depth = first.depths.slice();
  const depthVelocity = new Float32Array(COUNT);
  const shown = new Uint8Array(COUNT);
  const reached = new Uint32Array(COUNT);
  const waves: Wave[] = [];
  let waveCount = 0;
  let current = 0;
  let flashAt = -Infinity;

  for (let index = 0; index < COUNT; index++) {
    home[index * 2] = ((index % COLUMNS) - (COLUMNS - 1) / 2) * PITCH;
    home[index * 2 + 1] = ((ROWS - 1) / 2 - Math.floor(index / COLUMNS)) * PITCH;
    field.setColorAt(index, color.fromArray(first.colors, index * 3));
  }

  key.position.set(-4, 6, 8);
  scene.add(field, key, new THREE.AmbientLight(0xffffff, 0.35), lamp, flash);
  field.rotation.set(TILT.x, TILT.y, TILT.z);
  camera.position.set(0, -0.2, CAMERA_DISTANCE);
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  element.appendChild(renderer.domElement);

  void Promise.all(
    RENDERS.slice(1).map(async ({ src }, at) => {
      reliefs[at + 1] = await sampleRelief(THREE, src);
    }),
  );

  const raycaster = new THREE.Raycaster();
  const up = new THREE.Vector3(0, 0, 1);
  const plane = new THREE.Plane();
  const ray = new THREE.Vector2(OFF_FIELD, OFF_FIELD);
  const hit = new THREE.Vector3();
  const pointer = new THREE.Vector3(OFF_FIELD, OFF_FIELD, 0);

  /** The field-space point under `ndc`, or `undefined` when it misses the field's plane. */
  const project = (ndc: { x: number; y: number }) => {
    field.updateMatrixWorld();
    plane.set(up, 0).applyMatrix4(field.matrixWorld);
    raycaster.setFromCamera(ray.set(ndc.x, ndc.y), camera);
    return raycaster.ray.intersectPlane(plane, hit) ? field.worldToLocal(hit) : undefined;
  };
  const toNdc = (event: { clientX: number; clientY: number }) => {
    const rect = element.getBoundingClientRect();

    return {
      x: ((event.clientX - rect.left) / rect.width) * 2 - 1,
      y: -((event.clientY - rect.top) / rect.height) * 2 + 1,
    };
  };

  let pointerNdc: { x: number; y: number } | undefined;
  let frame = 0;
  let visible = true;
  const start = performance.now();
  let last = start;

  const scrollTilt = () => {
    const { top, height } = element.getBoundingClientRect();

    return Math.min(Math.max(-top / height, 0), 1);
  };

  const applyRender = (index: number, render: number) => {
    const relief = reliefs[render];

    if (!relief) {
      return;
    }

    shown[index] = render;
    field.setColorAt(index, color.fromArray(relief.colors, index * 3));
  };

  /** Moves every block one frame and draws; returns whether anything is still moving. */
  const step = (now: number) => {
    const dt = Math.min((now - last) / 1000, 1 / 30);
    const elapsed = reduceMotion ? Infinity : now - start;
    const tilt = reduceMotion ? 0 : scrollTilt();
    let moving = elapsed < SCAN_MS + DROP_MS || waves.length > 0;
    let recolored = false;

    last = now;
    field.rotation.set(
      TILT.x - tilt * 0.55,
      TILT.y + (pointerNdc ? pointerNdc.x * 0.04 : 0),
      TILT.z + tilt * 0.12,
    );

    const local = pointerNdc && project(pointerNdc);

    pointer.set(local?.x ?? OFF_FIELD, local?.y ?? OFF_FIELD, 0);
    lamp.position.copy(field.localToWorld(hit.copy(pointer).setZ(1.4)));
    lamp.intensity = local ? 28 : 0;
    flash.intensity = 90 * Math.max(1 - (now - flashAt) / FLASH_MS, 0) ** 2;

    for (let at = waves.length - 1; at >= 0; at--) {
      const wave = waves[at];

      if (wave && ((now - wave.start) / 1000) * WAVE_SPEED > WAVE_REACH) {
        waves.splice(at, 1);
      }
    }

    for (let index = 0; index < COUNT; index++) {
      const x = home[index * 2] ?? 0;
      const y = home[index * 2 + 1] ?? 0;
      const distance = Math.hypot(x - pointer.x, y - pointer.y);
      const target = distance < POINTER_RADIUS ? (1 - distance / POINTER_RADIUS) ** 2 * 1.6 : 0;
      let ring = 0;

      for (const wave of waves) {
        const age = (now - wave.start) / 1000;
        const behind = Math.hypot(x - wave.x, y - wave.y) - age * WAVE_SPEED;

        ring +=
          WAVE_HEIGHT *
          Math.exp(-((behind / WAVE_WIDTH) ** 2)) *
          Math.exp(-age / WAVE_DECAY_S) *
          Math.min(age * 6, 1);
        if (behind <= 0 && wave.order > (reached[index] ?? 0)) {
          reached[index] = wave.order;
          applyRender(index, wave.render);
          recolored = true;
        }
      }

      const goal = reliefs[shown[index] ?? 0]?.depths[index] ?? 0;
      const settle = easeOut((elapsed - (index / COUNT) * SCAN_MS) / DROP_MS);

      liftVelocity[index] =
        ((liftVelocity[index] ?? 0) + (target - (lift[index] ?? 0)) * 140 * dt) * (1 - 14 * dt);
      lift[index] = (lift[index] ?? 0) + (liftVelocity[index] ?? 0) * dt;
      depthVelocity[index] =
        ((depthVelocity[index] ?? 0) + (goal - (depth[index] ?? 0)) * 220 * dt) * (1 - 9 * dt);
      depth[index] = reduceMotion ? goal : (depth[index] ?? 0) + (depthVelocity[index] ?? 0) * dt;

      const blockDepth = Math.max(depth[index] ?? 0, 0.02);

      if (Math.abs(liftVelocity[index] ?? 0) > 1e-3 || Math.abs(depthVelocity[index] ?? 0) > 1e-3) {
        moving = true;
      }
      position.set(x, y, blockDepth / 2 + (lift[index] ?? 0) + ring + (1 - settle) * 6);
      scale.set(1, 1, Math.max(blockDepth * settle, 0.001));
      field.setMatrixAt(index, matrix.compose(position, rotation, scale));
    }

    field.instanceMatrix.needsUpdate = true;
    if (recolored && field.instanceColor) {
      field.instanceColor.needsUpdate = true;
    }
    renderer.render(scene, camera);
    return moving || local !== undefined || now - flashAt < FLASH_MS;
  };

  const draw = (now: number) => {
    const moving = step(now);

    frame = moving && visible && !document.hidden ? requestAnimationFrame(draw) : 0;
  };

  const resume = () => {
    if (reduceMotion) {
      step(performance.now());
    } else if (frame === 0 && visible && !document.hidden) {
      last = performance.now();
      frame = requestAnimationFrame(draw);
    }
  };

  const resize = () => {
    const { width, height } = element.getBoundingClientRect();

    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.position.z = CAMERA_DISTANCE * Math.max(1, 1.9 / camera.aspect);
    camera.updateProjectionMatrix();
    resume();
  };
  const resizeObserver = new ResizeObserver(resize);
  const intersection = new IntersectionObserver(([entry]) => {
    visible = entry?.isIntersecting ?? false;
    resume();
  });
  const onVisibility = () => {
    if (document.hidden) {
      cancelAnimationFrame(frame);
      frame = 0;
    } else {
      resume();
    }
  };
  const onPointerMove = (event: PointerEvent) => {
    pointerNdc = toNdc(event);
    resume();
  };
  const onPointerLeave = () => {
    pointerNdc = undefined;
    resume();
  };

  const next: Field["next"] = (ndc) => {
    const render = (current + 1) % RENDERS.length;

    if (!reliefs[render]) {
      return;
    }

    current = render;
    onRender(render);
    if (reduceMotion) {
      for (let index = 0; index < COUNT; index++) {
        applyRender(index, render);
      }
      if (field.instanceColor) {
        field.instanceColor.needsUpdate = true;
      }
      resume();
      return;
    }

    const origin = (ndc && project(ndc)) ?? new THREE.Vector3();

    waves.push({ x: origin.x, y: origin.y, start: performance.now(), order: ++waveCount, render });
    flash.position.copy(field.localToWorld(hit.copy(origin).setZ(1.2)));
    flashAt = performance.now();
    resume();
  };
  const onClick = (event: MouseEvent) => next(toNdc(event));

  resizeObserver.observe(element);
  intersection.observe(element);
  document.addEventListener("visibilitychange", onVisibility);
  element.addEventListener("click", onClick);
  if (finePointer && !reduceMotion) {
    element.addEventListener("pointermove", onPointerMove, { passive: true });
    element.addEventListener("pointerleave", onPointerLeave);
  }
  if (!reduceMotion) {
    addEventListener("scroll", resume, { passive: true });
  }
  resume();

  return {
    next,
    dispose: () => {
      cancelAnimationFrame(frame);
      intersection.disconnect();
      resizeObserver.disconnect();
      document.removeEventListener("visibilitychange", onVisibility);
      element.removeEventListener("click", onClick);
      element.removeEventListener("pointermove", onPointerMove);
      element.removeEventListener("pointerleave", onPointerLeave);
      removeEventListener("scroll", resume);
      field.geometry.dispose();
      field.material.dispose();
      field.dispose();
      renderer.dispose();
      renderer.domElement.remove();
    },
  };
}

/**
 * Takumi renders as a field of physical pixels. They drop in scan order, the pointer lifts them,
 * and a click sends a wave that leaves the next render behind it.
 */
export function PixelField() {
  const host = useRef<HTMLDivElement>(null);
  const field = useRef<Field>(undefined);
  const [ready, setReady] = useState(false);
  const [render, setRender] = useState(0);

  useEffect(() => {
    const element = host.current;

    if (!element) {
      return;
    }

    let disposed = false;

    void import("three").then(async (THREE) => {
      const first = await sampleRelief(THREE, RENDERS[0]?.src ?? "");

      if (disposed) {
        return;
      }

      field.current = createField(THREE, element, first, setRender);
      setReady(true);
    });

    return () => {
      disposed = true;
      field.current?.dispose();
      field.current = undefined;
    };
  }, []);

  return (
    <>
      <div
        ref={host}
        aria-hidden
        data-ready={ready}
        className="absolute inset-0 cursor-crosshair opacity-0 transition-opacity duration-500 ease-out data-[ready=true]:opacity-100 [&>canvas]:size-full"
      />
      <figcaption className="pointer-events-none absolute inset-x-5 bottom-4 flex items-center justify-between gap-4 font-mono text-xs text-white/55">
        <span>
          {RENDERS[render]?.name}
          <span className="max-sm:hidden"> · rendered by Takumi</span>
        </span>
        <button
          type="button"
          disabled={!ready}
          onClick={() => field.current?.next()}
          className="pointer-events-auto inline-flex shrink-0 items-center gap-1.5 whitespace-nowrap rounded-full border border-white/15 px-3 py-1.5 text-white/80 transition-[background-color,color,transform] duration-150 ease-out hover:bg-white/10 hover:text-white active:scale-[0.97] disabled:opacity-40"
        >
          Next render
          <ArrowRight className="size-3.5" />
        </button>
      </figcaption>
    </>
  );
}
