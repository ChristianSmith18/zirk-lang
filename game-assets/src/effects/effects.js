import * as THREE from 'three';
import { PALETTE } from '../palette.js';
import { glow, ghost } from '../materials.js';
import { Asset, box, cone, cylinder, torus, mesh } from '../shapes.js';

// Particle bursts are seeded so tests and screenshots are reproducible.
function lcg(seed) {
  let s = seed >>> 0;
  return () => ((s = (Math.imul(s, 1664525) + 1013904223) >>> 0) / 4294967296);
}

/** A one-shot burst of sparkles (coin pickup, goal reached). `finished` flips when done. */
export function createSparkles({ color = PALETTE.warning, count = 28, lifetime = 0.9, seed = 1 } = {}) {
  const fx = new Asset('sparkles');
  const rand = lcg(seed);
  const positions = new Float32Array(count * 3);
  const velocities = [];
  for (let i = 0; i < count; i++) {
    const angle = rand() * Math.PI * 2;
    const speed = 0.5 + rand() * 1.1;
    velocities.push(new THREE.Vector3(Math.cos(angle) * speed, 0.8 + rand() * 1.4, Math.sin(angle) * speed));
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  const material = new THREE.PointsMaterial({ color, size: 0.09, transparent: true, opacity: 1, depthWrite: false, sizeAttenuation: true });
  const points = new THREE.Points(geometry, material);
  // The positions are rewritten every frame, so the bounding sphere computed
  // for the initial all-zero burst would cull the sparkles once they spread.
  points.frustumCulled = false;
  fx.add(points);
  fx.finished = false;
  fx.animate = (dt, t) => {
    const p = Math.min(t / lifetime, 1);
    for (let i = 0; i < count; i++) {
      positions[i * 3] = velocities[i].x * t;
      positions[i * 3 + 1] = 0.3 + velocities[i].y * t - 2.2 * t * t;
      positions[i * 3 + 2] = velocities[i].z * t;
    }
    geometry.attributes.position.needsUpdate = true;
    material.opacity = 1 - p;
    fx.finished = p >= 1;
    points.visible = !fx.finished;
  };
  fx.animate(0, 0);
  fx.dispose = () => {
    geometry.dispose();
    material.dispose();
  };
  return fx;
}

/** A blob shadow under characters and items (cheaper than real shadows). */
export function createShadowBlob({ radius = 0.3, opacity = 0.35 } = {}) {
  const shadow = new Asset('shadow-blob');
  const disc = new THREE.Mesh(new THREE.CircleGeometry(radius, 20), new THREE.MeshBasicMaterial({ color: 0x000000, transparent: true, opacity, depthWrite: false }));
  disc.rotation.x = -Math.PI / 2;
  disc.position.y = 0.012;
  shadow.add(disc);
  shadow.dispose = () => {
    disc.geometry.dispose();
    disc.material.dispose();
  };
  return shadow;
}

/** A ring that marks the object or tile the code is talking about. */
export function createHighlightRing({ color = PALETTE.cyan, radius = 0.5 } = {}) {
  const ring = new Asset('highlight-ring');
  const inner = mesh(torus(radius, 0.02, 40), glow(color, 1.8), [0, 0.03, 0], { rotation: [Math.PI / 2, 0, 0], shadow: false });
  ring.add(inner);
  ring.add(mesh(torus(radius + 0.09, 0.008, 40), ghost(color, 0.5), [0, 0.03, 0], { rotation: [Math.PI / 2, 0, 0], shadow: false }));
  ring.animate = (dt, t) => {
    inner.scale.setScalar(1 + Math.sin(t * 4) * 0.05);
  };
  return ring;
}

/** An arrow on the floor showing the next step of a traced path. */
export function createPathArrow({ color = PALETTE.cyan } = {}) {
  const arrow = new Asset('path-arrow');
  arrow.add(mesh(box(0.1, 0.02, 0.3), glow(color, 1.4), [0, 0.02, -0.08], { shadow: false }));
  arrow.add(mesh(cone(0.16, 0.24, 3), glow(color, 1.4), [0, 0.02, 0.14], { rotation: [Math.PI / 2, 0, 0], scale: [1, 1, 0.12], shadow: false }));
  arrow.animate = (dt, t) => {
    arrow.position.z = Math.sin(t * 5) * 0.03;
  };
  return arrow;
}

/** A dot in the breadcrumb trail the character leaves behind. */
export function createTrailDot({ color = PALETTE.violet } = {}) {
  const dot = new Asset('trail-dot');
  dot.add(mesh(cylinder(0.07, 0.07, 0.02, 12), glow(color, 1.5), [0, 0.02, 0], { shadow: false }));
  return dot;
}
