import * as THREE from 'three';
import { PALETTE } from './palette.js';

/** Lights tuned for the flat-shaded look. Returns the group and the sun for shadow tuning. */
export function createLights({ shadows = true } = {}) {
  const group = new THREE.Group();
  group.name = 'lights';
  const hemi = new THREE.HemisphereLight(0xcfd6ff, PALETTE.deepViolet, 1.25);
  group.add(hemi);
  const sun = new THREE.DirectionalLight(0xffffff, 2.1);
  sun.position.set(4, 8, 5);
  sun.castShadow = shadows;
  sun.shadow.mapSize.set(2048, 2048);
  sun.shadow.camera.left = -8;
  sun.shadow.camera.right = 8;
  sun.shadow.camera.top = 8;
  sun.shadow.camera.bottom = -8;
  sun.shadow.bias = -0.0004;
  group.add(sun);
  const rim = new THREE.DirectionalLight(PALETTE.violet, 0.9);
  rim.position.set(-5, 3, -4);
  group.add(rim);
  return { group, sun, hemi, rim };
}

/** A vertical gradient dome: deep violet at the horizon up to near-black. */
export function createSky({ top = PALETTE.background, horizon = PALETTE.deepViolet } = {}) {
  const geometry = new THREE.SphereGeometry(60, 24, 16);
  const colors = [];
  const position = geometry.attributes.position;
  const a = new THREE.Color(top);
  const b = new THREE.Color(horizon);
  for (let i = 0; i < position.count; i++) {
    const t = THREE.MathUtils.clamp((position.getY(i) + 10) / 50, 0, 1);
    const c = b.clone().lerp(a, t);
    colors.push(c.r, c.g, c.b);
  }
  geometry.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
  const sky = new THREE.Mesh(geometry, new THREE.MeshBasicMaterial({ vertexColors: true, side: THREE.BackSide, fog: false, depthWrite: false }));
  sky.name = 'sky';
  return sky;
}

/** Sparse stars so the dark background is not flat. Seeded and cheap. */
export function createStars({ count = 220, seed = 7 } = {}) {
  let s = seed >>> 0;
  const rand = () => ((s = (Math.imul(s, 1664525) + 1013904223) >>> 0) / 4294967296);
  const positions = new Float32Array(count * 3);
  for (let i = 0; i < count; i++) {
    const u = rand() * Math.PI * 2;
    const v = 0.12 + rand() * 1.2;
    const r = 50;
    positions[i * 3] = Math.cos(u) * Math.cos(v) * r;
    positions[i * 3 + 1] = Math.sin(v) * r;
    positions[i * 3 + 2] = Math.sin(u) * Math.cos(v) * r;
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  const stars = new THREE.Points(geometry, new THREE.PointsMaterial({ color: PALETTE.text, size: 0.22, sizeAttenuation: true, transparent: true, opacity: 0.8, fog: false }));
  stars.name = 'stars';
  return stars;
}
