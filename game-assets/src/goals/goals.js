import * as THREE from 'three';
import { PALETTE } from '../palette.js';
import { solid, glow, ghost } from '../materials.js';
import { Asset, box, cylinder, torus, cone, roundBox, plane, sphere, octa, mesh } from '../shapes.js';

/** The level exit: two pillars, spinning rings and a lit disc. */
export function createGoalPortal({ color = PALETTE.violet } = {}) {
  const portal = new Asset('goal-portal');
  portal.add(mesh(cylinder(0.46, 0.5, 0.1, 6), solid(PALETTE.deepViolet, { metalness: 0.4 }), [0, 0.05, 0]));
  const disc = mesh(cylinder(0.36, 0.36, 0.02, 24), glow(color, 1.6), [0, 0.11, 0], { shadow: false });
  portal.add(disc);
  const rings = [];
  for (let i = 0; i < 3; i++) {
    const ring = mesh(torus(0.28 + i * 0.07, 0.025, 28), glow(i === 1 ? PALETTE.cyan : color, 1.5), [0, 0.28 + i * 0.22, 0], { rotation: [Math.PI / 2, 0, 0], shadow: false });
    rings.push(ring);
    portal.add(ring);
  }
  for (const x of [-0.4, 0.4]) {
    portal.add(mesh(roundBox(0.1, 0.9, 0.1, 0.03), solid(PALETTE.raised, { metalness: 0.5 }), [x, 0.5, 0]));
    portal.add(mesh(box(0.06, 0.06, 0.06), glow(PALETTE.cyan, 1.6), [x, 0.98, 0], { shadow: false }));
  }
  const beam = mesh(cylinder(0.3, 0.3, 1.1, 20, 1), ghost(color, 0.18), [0, 0.65, 0], { shadow: false });
  portal.add(beam);
  portal.animate = (dt, t) => {
    rings.forEach((ring, i) => {
      ring.rotation.z = t * (1.2 + i * 0.5) * (i % 2 ? -1 : 1);
      ring.position.y = 0.28 + i * 0.22 + Math.sin(t * 2 + i) * 0.03;
    });
    beam.material.opacity = 0.14 + Math.sin(t * 3) * 0.05;
  };
  return portal;
}

/** A classic goal flag. `raise(progress)` lifts the cloth up the pole. */
export function createFlag({ color = PALETTE.pink } = {}) {
  const flag = new Asset('flag');
  flag.add(mesh(cylinder(0.05, 0.05, 1.1, 8), solid(PALETTE.text, { metalness: 0.4 }), [0, 0.55, 0]));
  flag.add(mesh(cylinder(0.16, 0.2, 0.08, 10), solid(PALETTE.stoneDark), [0, 0.04, 0]));
  flag.add(mesh(cone(0.06, 0.1, 8), glow(PALETTE.warning, 1.5), [0, 1.15, 0], { shadow: false }));
  const cloth = mesh(box(0.5, 0.3, 0.02), solid(color, { emissive: color, emissiveIntensity: 0.2 }), [0.27, 0.9, 0]);
  flag.add(cloth);
  flag.raise = (progress = 1) => {
    cloth.position.y = 0.3 + THREE.MathUtils.clamp(progress, 0, 1) * 0.6;
  };
  flag.animate = (dt, t) => {
    cloth.rotation.y = Math.sin(t * 4) * 0.18;
    cloth.scale.x = 1 + Math.sin(t * 5) * 0.04;
  };
  return flag;
}

/** Where a character starts: a hex pad with a soft pulsing ring. */
export function createStartPad({ color = PALETTE.cyan } = {}) {
  const pad = new Asset('start-pad');
  pad.add(mesh(cylinder(0.44, 0.46, 0.05, 6), solid(PALETTE.raised, { metalness: 0.4 }), [0, 0.025, 0]));
  pad.add(mesh(cylinder(0.32, 0.32, 0.02, 6), glow(color, 1.1), [0, 0.055, 0], { shadow: false }));
  // The ring fades as it expands, so it owns its material (a shared cached one
  // would fade every pad together).
  const ringMaterial = new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0.9, depthWrite: false });
  const ring = mesh(torus(0.38, 0.015, 36), ringMaterial, [0, 0.07, 0], { rotation: [Math.PI / 2, 0, 0], shadow: false });
  pad.add(ring);
  pad.animate = (dt, t) => {
    const phase = (t % 1.6) / 1.6;
    ring.scale.setScalar(1 + phase * 0.3);
    ringMaterial.opacity = 0.9 * (1 - phase);
  };
  pad.dispose = () => ringMaterial.dispose();
  return pad;
}

/** A checkpoint: a small beacon that lights when reached via `activate()`. */
export function createCheckpoint({ color = PALETTE.aqua } = {}) {
  const cp = new Asset('checkpoint');
  cp.add(mesh(cylinder(0.3, 0.34, 0.08, 8), solid(PALETTE.stoneDark), [0, 0.04, 0]));
  cp.add(mesh(cylinder(0.04, 0.05, 0.5, 8), solid(PALETTE.raised, { metalness: 0.5 }), [0, 0.33, 0]));
  const lamp = mesh(octa(0.11), solid(PALETTE.muted), [0, 0.68, 0]);
  cp.add(lamp);
  cp.active = false;
  cp.activate = (on = true) => {
    cp.active = on;
    lamp.material = on ? glow(color, 2.2) : solid(PALETTE.muted);
  };
  cp.animate = (dt, t) => {
    lamp.rotation.y = t * 1.5;
    lamp.position.y = 0.68 + (cp.active ? Math.sin(t * 3) * 0.04 : 0);
  };
  return cp;
}

/** A gate (the key's lock). `open(progress)` slides the bars down. */
export function createDoor({ color = PALETTE.pink } = {}) {
  const door = new Asset('door');
  for (const x of [-0.45, 0.45]) {
    door.add(mesh(roundBox(0.14, 1, 0.16, 0.04), solid(PALETTE.stone), [x, 0.5, 0]));
    door.add(mesh(box(0.08, 0.08, 0.08), glow(color, 1.5), [x, 1.04, 0], { shadow: false }));
  }
  door.add(mesh(roundBox(1, 0.12, 0.18, 0.04), solid(PALETTE.stoneDark), [0, 1.0, 0]));
  const bars = new THREE.Group();
  for (let i = -3; i <= 3; i++) bars.add(mesh(cylinder(0.025, 0.025, 0.92, 6), solid(color, { metalness: 0.5, roughness: 0.4 }), [i * 0.1, 0.46, 0]));
  bars.add(mesh(box(0.76, 0.05, 0.05), solid(color, { metalness: 0.5 }), [0, 0.5, 0]));
  door.add(bars);
  const lock = mesh(roundBox(0.16, 0.18, 0.1, 0.03), solid(PALETTE.warning, { metalness: 0.6 }), [0, 0.5, 0.1]);
  door.add(lock);
  door.open = (progress = 1) => {
    const p = THREE.MathUtils.clamp(progress, 0, 1);
    bars.position.y = -0.95 * p;
    lock.visible = p < 0.2;
  };
  return door;
}

/** A pressure plate: `press(on)` sinks it and lights it. */
export function createPlate({ color = PALETTE.cyan } = {}) {
  const plate = new Asset('plate');
  plate.add(mesh(roundBox(0.8, 0.06, 0.8, 0.03), solid(PALETTE.stoneDark), [0, 0.03, 0]));
  const top = new THREE.Group();
  const face = mesh(roundBox(0.6, 0.06, 0.6, 0.03), solid(PALETTE.raised, { metalness: 0.4 }), [0, 0.09, 0]);
  top.add(face);
  const light = mesh(box(0.34, 0.012, 0.34), glow(color, 0.5), [0, 0.125, 0], { shadow: false });
  top.add(light);
  plate.add(top);
  plate.press = (on = true) => {
    top.position.y = on ? -0.05 : 0;
    light.material = glow(color, on ? 2.2 : 0.5);
  };
  return plate;
}

/** A lever with a pivoting handle: `pull(progress)` goes 0..1. */
export function createLever({ color = PALETTE.pink } = {}) {
  const lever = new Asset('lever');
  lever.add(mesh(roundBox(0.46, 0.14, 0.3, 0.04), solid(PALETTE.stoneDark), [0, 0.07, 0]));
  const arm = new THREE.Group();
  arm.position.set(0, 0.14, 0);
  arm.add(mesh(cylinder(0.03, 0.03, 0.42, 8), solid(PALETTE.text, { metalness: 0.5 }), [0, 0.21, 0]));
  arm.add(mesh(sphere(0.075, 10), glow(color, 1.4), [0, 0.45, 0], { shadow: false }));
  lever.add(arm);
  lever.pull = (progress = 1) => {
    arm.rotation.z = THREE.MathUtils.lerp(0.7, -0.7, THREE.MathUtils.clamp(progress, 0, 1));
  };
  lever.pull(0);
  return lever;
}

/** A terminal screen on a stand, used by lessons that print text. */
export function createTerminal({ color = PALETTE.aqua } = {}) {
  const t = new Asset('terminal');
  t.add(mesh(roundBox(0.36, 0.1, 0.28, 0.03), solid(PALETTE.stoneDark), [0, 0.05, 0]));
  t.add(mesh(cylinder(0.04, 0.05, 0.28, 8), solid(PALETTE.raised), [0, 0.22, 0]));
  const frame = mesh(roundBox(0.72, 0.5, 0.1, 0.05), solid(PALETTE.raised, { metalness: 0.4 }), [0, 0.58, 0]);
  frame.rotation.x = -0.08;
  t.add(frame);
  const screen = new THREE.Group();
  screen.position.set(0, 0.58, 0.055);
  screen.rotation.x = -0.08;
  screen.add(mesh(plane(0.62, 0.4), solid(PALETTE.background, { roughness: 0.2, metalness: 0.2 }), [0, 0, 0], { shadow: false }));
  const lines = [];
  [0.12, 0.04, -0.04, -0.12].forEach((y, i) => {
    const w = [0.34, 0.46, 0.22, 0.4][i];
    const line = mesh(box(w, 0.025, 0.004), glow(color, 1.3), [-0.28 + w / 2 + 0.03 - 0.0, y, 0.004], { shadow: false });
    lines.push(line);
    screen.add(line);
  });
  t.add(screen);
  const cursor = mesh(box(0.04, 0.05, 0.004), glow(PALETTE.text, 1.6), [-0.22, -0.18, 0.006], { shadow: false });
  screen.add(cursor);
  t.animate = (dt, tm) => {
    cursor.visible = Math.floor(tm * 2) % 2 === 0;
  };
  return t;
}
