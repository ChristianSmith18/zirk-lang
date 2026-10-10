import * as THREE from 'three';
import { PALETTE } from '../palette.js';
import { solid, glow, ghost } from '../materials.js';
import { Asset, box, cylinder, octa, icosa, torus, roundBox, sphere, mesh } from '../shapes.js';

/** A spinning gold coin with a "Z" stamped on both faces. Collect = +1. */
export function createCoin({ color = PALETTE.gold } = {}) {
  const coin = new Asset('coin');
  const body = new THREE.Group();
  body.rotation.x = Math.PI / 2;
  body.add(mesh(cylinder(0.24, 0.24, 0.06, 20), solid(color, { metalness: 0.7, roughness: 0.3, emissive: color, emissiveIntensity: 0.25 })));
  body.add(mesh(torus(0.235, 0.025, 24), solid(color, { metalness: 0.8, roughness: 0.25 }), [0, 0, 0], { rotation: [Math.PI / 2, 0, 0] }));
  const stamp = glow(PALETTE.deepViolet, 0.2);
  for (const side of [1, -1]) {
    const z = new THREE.Group();
    z.position.set(0, side * 0.034, 0);
    z.rotation.x = side * Math.PI / 2;
    z.add(mesh(box(0.2, 0.035, 0.012), stamp, [0, 0.07, 0], { shadow: false }));
    z.add(mesh(box(0.2, 0.035, 0.012), stamp, [0, -0.07, 0], { shadow: false }));
    z.add(mesh(box(0.24, 0.035, 0.012), stamp, [0, 0, 0], { rotation: [0, 0, 0.9], shadow: false }));
    body.add(z);
  }
  coin.add(body);
  coin.position.y = 0.5;
  coin.animate = (dt, t) => {
    body.rotation.z = t * 2.4;
    coin.position.y = 0.5 + Math.sin(t * 2.2) * 0.05;
  };
  return coin;
}

/** A faceted gem; worth more than a coin and a good "score" icon. */
export function createGem({ color = PALETTE.cyan } = {}) {
  const gem = new Asset('gem');
  const core = mesh(octa(0.26), solid(color, { roughness: 0.15, metalness: 0.3, emissive: color, emissiveIntensity: 0.55 }), [0, 0, 0], { scale: [1, 1.4, 1] });
  gem.add(core);
  gem.add(mesh(octa(0.12), glow(PALETTE.text, 1.2), [0, 0, 0], { scale: [1, 1.4, 1], shadow: false }));
  gem.position.y = 0.55;
  gem.animate = (dt, t) => {
    core.rotation.y = t * 1.6;
    gem.position.y = 0.55 + Math.sin(t * 2) * 0.06;
  };
  return gem;
}

/** A key that opens a `door`. Pick it up, carry it, use it. */
export function createKey({ color = PALETTE.warning } = {}) {
  const key = new Asset('key');
  const metal = solid(color, { metalness: 0.7, roughness: 0.3, emissive: color, emissiveIntensity: 0.2 });
  const holder = new THREE.Group();
  holder.rotation.z = Math.PI / 4;
  holder.add(mesh(torus(0.11, 0.035, 18), metal, [-0.18, 0, 0]));
  holder.add(mesh(box(0.34, 0.06, 0.06), metal, [0.05, 0, 0]));
  holder.add(mesh(box(0.06, 0.12, 0.06), metal, [0.15, -0.08, 0]));
  holder.add(mesh(box(0.06, 0.09, 0.06), metal, [0.22, -0.065, 0]));
  key.add(holder);
  key.position.y = 0.5;
  key.animate = (dt, t) => {
    holder.rotation.y = t * 1.8;
    key.position.y = 0.5 + Math.sin(t * 2.4) * 0.05;
  };
  return key;
}

/** An energy cell. Lessons about numbers: the glow level shows the value. */
export function createBattery({ level = 1 } = {}) {
  const battery = new Asset('battery');
  battery.add(mesh(roundBox(0.3, 0.5, 0.3, 0.06), solid(PALETTE.stone, { roughness: 0.4, metalness: 0.4 }), [0, 0.25, 0]));
  battery.add(mesh(cylinder(0.07, 0.07, 0.07, 10), solid(PALETTE.muted, { metalness: 0.7 }), [0, 0.535, 0]));
  const fill = mesh(box(0.2, 0.36, 0.02), glow(PALETTE.aqua, 1.4), [0, 0.25, 0.155], { shadow: false });
  battery.add(fill);
  battery.setLevel = (value) => {
    const v = THREE.MathUtils.clamp(value, 0, 1);
    fill.scale.y = Math.max(v, 0.001);
    fill.position.y = 0.07 + 0.18 * v;
    fill.visible = v > 0;
  };
  battery.setLevel(level);
  return battery;
}

/** A treasure chest with a hinged lid: `open(progress)` takes 0..1. */
export function createChest({ color = PALETTE.wood } = {}) {
  const chest = new Asset('chest');
  const wood = solid(color);
  const band = solid(PALETTE.gold, { metalness: 0.6, roughness: 0.35 });
  chest.add(mesh(roundBox(0.62, 0.3, 0.42, 0.04), wood, [0, 0.15, 0]));
  chest.add(mesh(box(0.64, 0.32, 0.06), band, [0, 0.16, 0.14]));
  chest.add(mesh(box(0.64, 0.32, 0.06), band, [0, 0.16, -0.14]));
  const lid = new THREE.Group();
  lid.position.set(0, 0.3, -0.21);
  lid.add(mesh(cylinder(0.21, 0.21, 0.62, 12, 1), wood, [0, 0, 0.21], { rotation: [0, 0, Math.PI / 2], scale: [1, 1, 1] }));
  lid.add(mesh(box(0.1, 0.1, 0.04), band, [0, 0.02, 0.43]));
  chest.add(lid);
  const loot = mesh(sphere(0.12, 8), glow(PALETTE.gold, 1.6), [0, 0.3, 0], { shadow: false });
  loot.visible = false;
  chest.add(loot);
  chest.open = (progress = 1) => {
    lid.rotation.x = -THREE.MathUtils.clamp(progress, 0, 1) * 1.9;
    loot.visible = progress > 0.4;
    loot.position.y = 0.3 + progress * 0.18;
  };
  chest.open(0);
  return chest;
}

/** A data chip: the "value" token used by lessons about variables and types. */
export function createChip({ color = PALETTE.violet } = {}) {
  const chip = new Asset('chip');
  chip.add(mesh(roundBox(0.34, 0.06, 0.34, 0.03), solid(PALETTE.raised, { metalness: 0.5, roughness: 0.35 }), [0, 0, 0]));
  chip.add(mesh(roundBox(0.2, 0.07, 0.2, 0.02), glow(color, 1.1), [0, 0.01, 0], { shadow: false }));
  for (let i = -2; i <= 2; i++) {
    for (const [x, z, w, d] of [[i * 0.06, 0.2, 0.025, 0.07], [i * 0.06, -0.2, 0.025, 0.07], [0.2, i * 0.06, 0.07, 0.025], [-0.2, i * 0.06, 0.07, 0.025]]) {
      chip.add(mesh(box(w, 0.02, d), solid(PALETTE.gold, { metalness: 0.8, roughness: 0.3 }), [x, 0, z], { shadow: false }));
    }
  }
  chip.position.y = 0.4;
  chip.animate = (dt, t) => {
    chip.rotation.y = t * 1.2;
    chip.position.y = 0.4 + Math.sin(t * 2.2) * 0.04;
  };
  return chip;
}

/** A glowing orb used for "heart" / lives style counters. */
export function createOrb({ color = PALETTE.pink } = {}) {
  const orb = new Asset('orb');
  orb.add(mesh(icosa(0.2, 1), glow(color, 1.2), [0, 0, 0], { shadow: false }));
  orb.add(mesh(icosa(0.3, 1), ghost(color, 0.15), [0, 0, 0], { shadow: false }));
  orb.position.y = 0.5;
  orb.animate = (dt, t) => {
    orb.scale.setScalar(1 + Math.sin(t * 3) * 0.08);
    orb.position.y = 0.5 + Math.sin(t * 2) * 0.05;
  };
  return orb;
}
