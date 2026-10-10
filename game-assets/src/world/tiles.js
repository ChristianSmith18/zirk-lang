import * as THREE from 'three';
import { PALETTE } from '../palette.js';
import { solid, glow, ghost } from '../materials.js';
import { Asset, box, cylinder, cone, roundBox, torus, mesh } from '../shapes.js';

/** Tiles are 1x1 with the walkable surface at y = 0 and 0.2 of body below. */
export const TILE = Object.freeze({ size: 1, thickness: 0.2 });

// Deterministic variation: the same (type, variant) always looks the same.
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function slab(top, side = PALETTE.soil) {
  const group = new THREE.Group();
  // The body stops just under the surface plate so their top faces never
  // share a plane (that would z-fight).
  group.add(mesh(box(1, 0.17, 1), solid(side, { roughness: 0.9 }), [0, -0.115, 0]));
  group.add(mesh(roundBox(0.98, 0.06, 0.98, 0.025), solid(top, { roughness: 0.85 }), [0, -0.03, 0]));
  return group;
}

const BUILDERS = {
  plain(a, r) {
    a.add(slab(PALETTE.stone, PALETTE.stoneDark));
    if (r() > 0.5) a.add(mesh(box(0.3, 0.012, 0.3), solid(PALETTE.stoneDark), [(r() - 0.5) * 0.4, 0.002, (r() - 0.5) * 0.4], { shadow: false }));
  },
  circuit(a, r) {
    a.add(slab(PALETTE.raised, PALETTE.surface));
    const trace = glow(PALETTE.violet, 1.0);
    const y = 0.004;
    const turns = 2 + Math.floor(r() * 3);
    let x = -0.38;
    let z = (r() - 0.5) * 0.5;
    for (let i = 0; i < turns; i++) {
      const length = 0.18 + r() * 0.3;
      if (i % 2 === 0) {
        a.add(mesh(box(length, 0.012, 0.035), trace, [x + length / 2, y, z], { shadow: false }));
        x += length;
      } else {
        const dz = (r() - 0.5) * 0.5;
        a.add(mesh(box(0.035, 0.012, Math.abs(dz) + 0.035), trace, [x, y, z + dz / 2], { shadow: false }));
        z += dz;
      }
    }
    a.add(mesh(cylinder(0.055, 0.055, 0.014, 10), glow(PALETTE.cyan, 1.4), [x, y, z], { shadow: false }));
  },
  grass(a, r) {
    a.add(slab(PALETTE.grass, PALETTE.soil));
    const blade = solid(PALETTE.grassDark);
    for (let i = 0; i < 7; i++) {
      a.add(mesh(cone(0.035, 0.12 + r() * 0.08, 4), blade, [(r() - 0.5) * 0.85, 0.06, (r() - 0.5) * 0.85]));
    }
    if (r() > 0.75) a.add(mesh(cylinder(0.025, 0.025, 0.08, 5), solid(PALETTE.text), [(r() - 0.5) * 0.7, 0.04, (r() - 0.5) * 0.7]));
  },
  sand(a, r) {
    a.add(slab(PALETTE.sand, PALETTE.woodDark));
    for (let i = 0; i < 4; i++) a.add(mesh(box(0.22, 0.01, 0.03), solid(0xd2b06f), [(r() - 0.5) * 0.6, 0.003, (r() - 0.5) * 0.8], { shadow: false }));
  },
  ice(a, r) {
    a.add(slab(PALETTE.ice, PALETTE.cyan));
    const top = a.children[0].children[1];
    top.material = solid(PALETTE.ice, { roughness: 0.08, metalness: 0.25, emissive: PALETTE.cyan, emissiveIntensity: 0.1 });
    for (let i = 0; i < 3; i++) a.add(mesh(box(0.3 + r() * 0.2, 0.008, 0.012), glow(PALETTE.text, 0.6, 0.7), [(r() - 0.5) * 0.4, 0.004, (r() - 0.5) * 0.7], { rotation: [0, r() * 3, 0], shadow: false }));
  },
  water(a) {
    a.add(mesh(box(1, 0.16, 1), solid(PALETTE.soil, { roughness: 0.9 }), [0, -0.12, 0]));
    const surface = mesh(box(0.98, 0.04, 0.98), solid(PALETTE.water, { roughness: 0.1, metalness: 0.2, opacity: 0.85, emissive: PALETTE.water, emissiveIntensity: 0.25 }), [0, -0.06, 0], { shadow: false });
    a.add(surface);
    a.animate = (dt, t) => {
      surface.position.y = -0.06 + Math.sin(t * 1.8 + a.position.x * 1.3 + a.position.z) * 0.012;
    };
  },
  lava(a, r) {
    a.add(mesh(box(1, 0.16, 1), solid(PALETTE.stoneDark, { roughness: 0.9 }), [0, -0.12, 0]));
    a.add(mesh(box(0.98, 0.04, 0.98), solid(0xc9441a, { emissive: PALETTE.lava, emissiveIntensity: 0.55, roughness: 0.5 }), [0, -0.06, 0], { shadow: false }));
    const blobs = [];
    for (let i = 0; i < 4; i++) {
      const blob = mesh(cylinder(0.07 + r() * 0.05, 0.07, 0.03, 8), glow(PALETTE.warning, 1.1), [(r() - 0.5) * 0.7, -0.03, (r() - 0.5) * 0.7], { shadow: false });
      blob.userData.phase = r() * 6;
      blobs.push(blob);
      a.add(blob);
    }
    a.animate = (dt, t) => {
      for (const blob of blobs) blob.scale.setScalar(0.7 + Math.abs(Math.sin(t * 1.4 + blob.userData.phase)) * 0.6);
    };
  },
  hole(a) {
    // A square pit: rim on top, nothing underneath.
    for (const [x, z, w, d] of [[0, 0.41, 1, 0.18], [0, -0.41, 1, 0.18], [0.41, 0, 0.18, 0.64], [-0.41, 0, 0.18, 0.64]]) {
      a.add(mesh(box(w, 0.2, d), solid(PALETTE.stoneDark, { roughness: 0.9 }), [x, -0.1, z]));
    }
    a.add(mesh(box(0.64, 0.02, 0.64), solid(PALETTE.background), [0, -0.19, 0], { shadow: false }));
    a.add(mesh(box(0.64, 0.01, 0.64), glow(PALETTE.error, 0.35, 0.5), [0, -0.1, 0], { shadow: false }));
  },
  ramp(a) {
    a.add(slab(PALETTE.stone, PALETTE.stoneDark));
    const wedge = new THREE.Shape();
    wedge.moveTo(-0.5, 0);
    wedge.lineTo(0.5, 0);
    wedge.lineTo(0.5, 0.5);
    wedge.lineTo(-0.5, 0);
    const geometry = new THREE.ExtrudeGeometry(wedge, { depth: 0.98, bevelEnabled: false });
    geometry.translate(0, 0, -0.49);
    a.add(mesh(geometry, solid(PALETTE.stone), [0, 0, 0]));
    for (let i = 0; i < 4; i++) a.add(mesh(box(0.05, 0.012, 0.8), solid(PALETTE.stoneDark), [-0.35 + i * 0.22, 0.12 + i * 0.11, 0], { rotation: [0, 0, Math.atan2(0.5, 1)], shadow: false }));
  },
  bridge(a) {
    for (let i = -3; i <= 3; i++) a.add(mesh(box(0.96, 0.06, 0.12), solid(i % 2 ? PALETTE.wood : PALETTE.woodDark), [0, -0.03, i * 0.14]));
    for (const x of [-0.46, 0.46]) a.add(mesh(box(0.05, 0.05, 1), solid(PALETTE.woodDark), [x, 0.0, 0]));
  },
  conveyor(a) {
    a.add(slab(PALETTE.stoneDark, PALETTE.surface));
    const arrows = [];
    for (let i = 0; i < 3; i++) {
      const arrow = new THREE.Group();
      arrow.add(mesh(box(0.26, 0.012, 0.06), glow(PALETTE.cyan, 1.2), [0, 0.004, -0.06], { shadow: false }));
      arrow.add(mesh(cone(0.11, 0.18, 3), glow(PALETTE.cyan, 1.2), [0, 0.006, 0.07], { rotation: [Math.PI / 2, 0, 0], scale: [1, 1, 0.1], shadow: false }));
      arrow.userData.offset = i / 3;
      arrows.push(arrow);
      a.add(arrow);
    }
    a.animate = (dt, t) => {
      for (const arrow of arrows) arrow.position.z = -0.45 + ((t * 0.5 + arrow.userData.offset) % 1) * 0.9;
    };
  },
  teleporter(a) {
    a.add(slab(PALETTE.raised, PALETTE.surface));
    const ringA = mesh(torus(0.3, 0.025, 28), glow(PALETTE.violet, 1.6), [0, 0.03, 0], { rotation: [Math.PI / 2, 0, 0], shadow: false });
    const ringB = mesh(torus(0.2, 0.02, 24), glow(PALETTE.pink, 1.6), [0, 0.03, 0], { rotation: [Math.PI / 2, 0, 0], shadow: false });
    a.add(ringA, ringB);
    a.add(mesh(cylinder(0.28, 0.28, 0.01, 24), ghost(PALETTE.violet, 0.35), [0, 0.02, 0], { shadow: false }));
    a.animate = (dt, t) => {
      ringA.rotation.z = t * 1.4;
      ringB.rotation.z = -t * 2.2;
      ringA.position.y = 0.03 + Math.sin(t * 3) * 0.02;
    };
  },
  spikes(a) {
    a.add(slab(PALETTE.stoneDark, PALETTE.surface));
    const spikes = new THREE.Group();
    for (let x = -1; x <= 1; x++) for (let z = -1; z <= 1; z++) {
      spikes.add(mesh(cone(0.07, 0.3, 4), solid(PALETTE.text, { metalness: 0.7, roughness: 0.3 }), [x * 0.27, 0.15, z * 0.27]));
    }
    a.add(spikes);
    a.retract = (progress = 1) => {
      spikes.scale.y = Math.max(0.05, 1 - THREE.MathUtils.clamp(progress, 0, 1));
    };
  },
};

export const TILE_TYPES = Object.freeze(Object.keys(BUILDERS));

/**
 * One floor tile. `variant` seeds the small details so neighbours differ but a
 * given (type, variant) is always identical.
 */
export function createTile(type = 'plain', { variant = 0 } = {}) {
  const build = BUILDERS[type];
  if (!build) throw new Error(`unknown tile "${type}"; expected one of ${TILE_TYPES.join(', ')}`);
  const tile = new Asset(`tile:${type}`);
  build(tile, rng(variant * 7919 + type.length * 131 + 17));
  tile.traverse((o) => {
    if (o.isMesh) o.receiveShadow = true;
  });
  return tile;
}
