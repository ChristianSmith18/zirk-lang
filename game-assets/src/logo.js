import * as THREE from 'three';
import { mergeGeometries } from 'three/addons/utils/BufferGeometryUtils.js';

/**
 * The Zirk mark, rebuilt in 3D from the original vector
 * (`docs/assets/zirk-clean.svg`, 48x48 view box): a stroked "Z" (top bar,
 * diagonal, bottom bar) crossed by a steep diagonal slash, both with round
 * caps and joins and a 5.25 stroke. The Z runs violet -> pink -> blue and the
 * slash blue -> green, exactly as the SVG gradients do (objectBoundingBox).
 *
 * The SVG remains the authority for the brand; if it changes, update the
 * constants below. Paths are in SVG coordinates (y grows downward).
 */
export const ZIRK_MARK = Object.freeze({
  viewBox: 48,
  stroke: 5.25,
  z: [
    [12.8, 12],
    [32.8, 12],
    [14.4, 36],
    [35.2, 36],
  ],
  slash: [
    [16, 8],
    [32, 40],
  ],
  zStops: [
    [0, '#a78bfa'],
    [0.4, '#ec4899'],
    [1, '#38bdf8'],
  ],
  slashStops: [
    [0, '#60a5fa'],
    [1, '#34d399'],
  ],
});

function sampleStops(stops, t) {
  const clamped = THREE.MathUtils.clamp(t, 0, 1);
  for (let i = 1; i < stops.length; i++) {
    if (clamped <= stops[i][0]) {
      const [t0, c0] = stops[i - 1];
      const [t1, c1] = stops[i];
      return new THREE.Color(c0).lerp(new THREE.Color(c1), (clamped - t0) / (t1 - t0));
    }
  }
  return new THREE.Color(stops[stops.length - 1][1]);
}

/** Builds the stroked polyline as boxes plus round joints, coloured by a gradient. */
function stroke(points, { width, depth, colorAt, k }) {
  const parts = [];
  const place = (geometry, x, y, rotationZ = 0) => {
    geometry.rotateZ(rotationZ);
    geometry.translate((x - 24) * k, -(y - 24) * k, 0);
    return geometry;
  };
  for (let i = 0; i < points.length - 1; i++) {
    const [x0, y0] = points[i];
    const [x1, y1] = points[i + 1];
    const length = Math.hypot(x1 - x0, y1 - y0);
    // SVG y points down, so the segment angle is measured in that frame and
    // negated for the Y-up scene.
    const angle = -Math.atan2(y1 - y0, x1 - x0);
    parts.push(place(new THREE.BoxGeometry(length * k, width * k, depth), (x0 + x1) / 2, (y0 + y1) / 2, angle));
  }
  // Round caps and joins: a short cylinder along Z at every vertex.
  for (const [x, y] of points) {
    parts.push(place(new THREE.CylinderGeometry((width / 2) * k, (width / 2) * k, depth, 16).rotateX(Math.PI / 2), x, y));
  }
  for (const part of parts) {
    const position = part.attributes.position;
    const colors = new Float32Array(position.count * 3);
    for (let i = 0; i < position.count; i++) {
      // Back to SVG coordinates to sample the gradient.
      const x = position.getX(i) / k + 24;
      const y = -position.getY(i) / k + 24;
      const c = colorAt(x, y);
      colors.set([c.r, c.g, c.b], i * 3);
    }
    part.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  }
  return mergeGeometries(parts);
}

const bbox = (points, half) => {
  const xs = points.map((p) => p[0]);
  const ys = points.map((p) => p[1]);
  return { x0: Math.min(...xs), y0: Math.min(...ys), w: Math.max(...xs) - Math.min(...xs), h: Math.max(...ys) - Math.min(...ys), half };
};

let cached;

/**
 * Geometries for the two strokes of the mark, centred on the origin in the XY
 * plane, facing +Z. `size` is the height of the whole mark in scene units.
 */
export function createZirkMarkGeometries({ size = 0.3, depth = 0.012 } = {}) {
  const key = `${size}:${depth}`;
  if (cached?.key === key) return cached.value;
  const { z, slash, stroke: width, zStops, slashStops } = ZIRK_MARK;
  const k = size / 32; // the mark spans y = 8..40 in the 48x48 box
  const zb = bbox(z);
  const sb = bbox(slash);
  // objectBoundingBox gradients: Z runs corner to corner top-left -> bottom-right,
  // the slash bottom-left -> top-right.
  const zGeometry = stroke(z, {
    width,
    depth,
    k,
    colorAt: (x, y) => sampleStops(zStops, ((x - zb.x0) / zb.w + (y - zb.y0) / zb.h) / 2),
  });
  const slashGeometry = stroke(slash, {
    width,
    depth,
    k,
    colorAt: (x, y) => sampleStops(slashStops, ((x - sb.x0) / sb.w + (1 - (y - sb.y0) / sb.h)) / 2),
  });
  const value = { z: zGeometry, slash: slashGeometry };
  cached = { key, value };
  return value;
}

export function disposeLogoGeometry() {
  if (cached) {
    cached.value.z.dispose();
    cached.value.slash.dispose();
    cached = undefined;
  }
}
