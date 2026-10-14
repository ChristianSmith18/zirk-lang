import * as THREE from 'three';
import { PALETTE } from '../palette.js';
import { solid, glow } from '../materials.js';
import { Asset, box, cylinder, cone, icosa, octa, roundBox, sphere, mesh } from '../shapes.js';

/** Solid wall block; `height` in tiles. */
export function createWall({ height = 1, color = PALETTE.stone } = {}) {
  const wall = new Asset('wall');
  wall.add(mesh(roundBox(0.98, height, 0.98, 0.05), solid(color, { roughness: 0.9 }), [0, height / 2, 0]));
  for (let i = 0; i < Math.round(height * 2); i++) {
    wall.add(mesh(box(1.0, 0.025, 1.0), solid(PALETTE.stoneDark), [0, 0.25 + i * 0.5, 0], { shadow: false }));
  }
  wall.add(mesh(roundBox(1.0, 0.07, 1.0, 0.03), solid(PALETTE.raised), [0, height, 0]));
  return wall;
}

export function createPillar({ height = 1.4 } = {}) {
  const pillar = new Asset('pillar');
  pillar.add(mesh(cylinder(0.3, 0.34, 0.12, 8), solid(PALETTE.stoneDark), [0, 0.06, 0]));
  pillar.add(mesh(cylinder(0.22, 0.25, height, 8), solid(PALETTE.stone), [0, height / 2 + 0.06, 0]));
  pillar.add(mesh(cylinder(0.3, 0.24, 0.12, 8), solid(PALETTE.stoneDark), [0, height + 0.12, 0]));
  pillar.add(mesh(octa(0.1), glow(PALETTE.violet, 1.5), [0, height + 0.3, 0], { shadow: false }));
  return pillar;
}

/** A pushable crate. */
export function createCrate({ color = PALETTE.wood } = {}) {
  const crate = new Asset('crate');
  const wood = solid(color);
  const frame = solid(PALETTE.woodDark);
  crate.add(mesh(roundBox(0.7, 0.7, 0.7, 0.04), wood, [0, 0.35, 0]));
  for (const x of [-0.33, 0.33]) for (const z of [-0.33, 0.33]) crate.add(mesh(box(0.06, 0.74, 0.06), frame, [x, 0.35, z]));
  for (const y of [0.04, 0.66]) crate.add(mesh(box(0.74, 0.06, 0.74), frame, [0, y, 0]));
  crate.add(mesh(box(0.5, 0.05, 0.02), frame, [0, 0.35, 0.36], { rotation: [0, 0, 0.7] }));
  crate.add(mesh(box(0.5, 0.05, 0.02), frame, [0, 0.35, 0.36], { rotation: [0, 0, -0.7] }));
  return crate;
}

export function createTree({ variant = 0 } = {}) {
  const tree = new Asset('tree');
  const tall = 0.5 + (variant % 3) * 0.12;
  tree.add(mesh(cylinder(0.08, 0.11, tall, 6), solid(PALETTE.woodDark), [0, tall / 2, 0]));
  const leaf = solid(PALETTE.leaf);
  tree.add(mesh(icosa(0.38), leaf, [0, tall + 0.22, 0]));
  tree.add(mesh(icosa(0.26), solid(PALETTE.grassDark), [0.22, tall + 0.08, 0.1]));
  tree.add(mesh(icosa(0.22), leaf, [-0.2, tall + 0.14, -0.1]));
  return tree;
}

export function createPine() {
  const pine = new Asset('pine');
  pine.add(mesh(cylinder(0.07, 0.09, 0.3, 6), solid(PALETTE.woodDark), [0, 0.15, 0]));
  [[0.36, 0.45, 0.4], [0.3, 0.4, 0.68], [0.22, 0.34, 0.94]].forEach(([r, h, y], i) => {
    pine.add(mesh(cone(r, h, 7), solid(i % 2 ? PALETTE.grassDark : PALETTE.leaf), [0, y, 0]));
  });
  return pine;
}

export function createRock({ variant = 0 } = {}) {
  const rock = new Asset('rock');
  const s = 0.85 + (variant % 3) * 0.2;
  rock.add(mesh(icosa(0.3 * s), solid(PALETTE.stone, { roughness: 0.95 }), [0, 0.2 * s, 0], { scale: [1, 0.7, 0.9] }));
  rock.add(mesh(icosa(0.18 * s), solid(PALETTE.stoneDark, { roughness: 0.95 }), [0.26 * s, 0.1 * s, 0.1], { scale: [1, 0.7, 1] }));
  return rock;
}

export function createCrystal({ color = PALETTE.cyan } = {}) {
  const crystal = new Asset('crystal');
  const mat = solid(color, { roughness: 0.15, emissive: color, emissiveIntensity: 0.6 });
  [[0, 0, 0.7, 0.14], [0.2, 0.08, 0.4, 0.1], [-0.18, -0.05, 0.5, 0.11], [0.05, -0.2, 0.3, 0.08]].forEach(([x, z, h, r], i) => {
    crystal.add(mesh(cone(r, h, 5), mat, [x, h / 2, z], { rotation: [i % 2 ? 0.12 : -0.1, 0, i % 2 ? -0.12 : 0.1] }));
  });
  crystal.animate = (dt, t) => {
    mat.emissiveIntensity = 0.5 + Math.sin(t * 2) * 0.15;
  };
  return crystal;
}

export function createLamp({ color = PALETTE.warning } = {}) {
  const lamp = new Asset('lamp');
  lamp.add(mesh(cylinder(0.14, 0.18, 0.08, 8), solid(PALETTE.stoneDark), [0, 0.04, 0]));
  lamp.add(mesh(cylinder(0.03, 0.04, 1.0, 6), solid(PALETTE.stone, { metalness: 0.3 }), [0, 0.58, 0]));
  lamp.add(mesh(sphere(0.11, 10), glow(color, 2.2), [0, 1.16, 0], { shadow: false }));
  lamp.add(mesh(cone(0.17, 0.1, 8), solid(PALETTE.raised, { metalness: 0.5 }), [0, 1.3, 0]));
  return lamp;
}

/** A signpost; the board is blank so the game can overlay text. */
export function createSign() {
  const sign = new Asset('sign');
  sign.add(mesh(box(0.07, 0.6, 0.07), solid(PALETTE.woodDark), [0, 0.3, 0]));
  sign.add(mesh(roundBox(0.7, 0.4, 0.06, 0.03), solid(PALETTE.wood), [0, 0.7, 0]));
  sign.add(mesh(box(0.52, 0.04, 0.02), glow(PALETTE.violet, 0.8), [0, 0.76, 0.035], { shadow: false }));
  sign.add(mesh(box(0.36, 0.04, 0.02), glow(PALETTE.cyan, 0.8), [-0.08, 0.66, 0.035], { shadow: false }));
  return sign;
}

export function createFence() {
  const fence = new Asset('fence');
  for (const x of [-0.45, 0, 0.45]) fence.add(mesh(box(0.07, 0.42, 0.07), solid(PALETTE.woodDark), [x, 0.21, 0]));
  for (const y of [0.16, 0.32]) fence.add(mesh(box(1, 0.06, 0.04), solid(PALETTE.wood), [0, y, 0]));
  return fence;
}

export function createBush() {
  const bush = new Asset('bush');
  bush.add(mesh(icosa(0.26), solid(PALETTE.grassDark), [0, 0.2, 0]));
  bush.add(mesh(icosa(0.2), solid(PALETTE.leaf), [0.2, 0.15, 0.08]));
  bush.add(mesh(icosa(0.18), solid(PALETTE.leaf), [-0.18, 0.14, -0.06]));
  bush.add(mesh(sphere(0.04, 6), solid(PALETTE.pink), [0.05, 0.36, 0.2]));
  return bush;
}

export function createBanner({ color = PALETTE.violet } = {}) {
  const banner = new Asset('banner');
  banner.add(mesh(cylinder(0.025, 0.025, 1.1, 6), solid(PALETTE.stone, { metalness: 0.3 }), [0, 0.55, 0]));
  banner.add(mesh(box(0.5, 0.04, 0.04), solid(PALETTE.stone, { metalness: 0.3 }), [0.2, 1.05, 0]));
  const cloth = mesh(box(0.34, 0.6, 0.02), solid(color, { emissive: color, emissiveIntensity: 0.15 }), [0.22, 0.74, 0]);
  banner.add(cloth);
  banner.add(mesh(box(0.14, 0.14, 0.022), glow(PALETTE.text, 1), [0.22, 0.78, 0], { rotation: [0, 0, 0.78], shadow: false }));
  banner.animate = (dt, t) => {
    cloth.rotation.x = Math.sin(t * 2.4) * 0.07;
  };
  return banner;
}
