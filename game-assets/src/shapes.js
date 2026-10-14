import * as THREE from 'three';
import { RoundedBoxGeometry } from 'three/addons/geometries/RoundedBoxGeometry.js';

// Geometry is cached by dimensions for the same reason materials are.
const cache = new Map();
const memo = (key, make) => {
  let geometry = cache.get(key);
  if (!geometry) {
    geometry = make();
    cache.set(key, geometry);
  }
  return geometry;
};

export const box = (w, h, d) => memo(`b:${w}:${h}:${d}`, () => new THREE.BoxGeometry(w, h, d));
export const roundBox = (w, h, d, r = 0.06) =>
  memo(`r:${w}:${h}:${d}:${r}`, () => new RoundedBoxGeometry(w, h, d, 2, Math.min(r, w / 2, h / 2, d / 2)));
export const cylinder = (rTop, rBottom, h, seg = 12) =>
  memo(`c:${rTop}:${rBottom}:${h}:${seg}`, () => new THREE.CylinderGeometry(rTop, rBottom, h, seg));
export const cone = (r, h, seg = 6) => memo(`n:${r}:${h}:${seg}`, () => new THREE.ConeGeometry(r, h, seg));
export const sphere = (r, seg = 12) => memo(`p:${r}:${seg}`, () => new THREE.SphereGeometry(r, seg, Math.max(6, seg >> 1)));
export const octa = (r) => memo(`o:${r}`, () => new THREE.OctahedronGeometry(r, 0));
export const icosa = (r, detail = 0) => memo(`i:${r}:${detail}`, () => new THREE.IcosahedronGeometry(r, detail));
export const torus = (r, tube, seg = 24) => memo(`t:${r}:${tube}:${seg}`, () => new THREE.TorusGeometry(r, tube, 8, seg));
export const plane = (w, h) => memo(`l:${w}:${h}`, () => new THREE.PlaneGeometry(w, h));

export function disposeGeometryCache() {
  for (const geometry of cache.values()) geometry.dispose();
  cache.clear();
}

/** Builds a mesh and optionally positions it, in one call. */
export function mesh(geometry, material, position = [0, 0, 0], { rotation, scale, shadow = true } = {}) {
  const m = new THREE.Mesh(geometry, material);
  m.position.set(...position);
  if (rotation) m.rotation.set(...rotation);
  if (scale) m.scale.set(...(Array.isArray(scale) ? scale : [scale, scale, scale]));
  m.castShadow = shadow;
  m.receiveShadow = true;
  return m;
}

/** A Group that can carry a per-frame `update(dt)` and a few shared flags. */
export class Asset extends THREE.Group {
  constructor(kind) {
    super();
    this.name = kind;
    this.userData.kind = kind;
    this._time = 0;
  }
  /** Advance the asset's own animation by `dt` seconds. */
  update(dt) {
    this._time += dt;
    this.animate?.(dt, this._time);
  }
}
