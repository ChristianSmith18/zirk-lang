import * as THREE from 'three';

// Materials are cached by their parameters: a level with hundreds of tiles
// shares a handful of GPU programs. `dispose()` on a cloned asset therefore
// must not dispose materials; call `disposeMaterialCache()` once at teardown.
const cache = new Map();

/** Flat-shaded standard material, the look of every asset. */
export function solid(color, { roughness = 0.62, metalness = 0.08, emissive = 0x000000, emissiveIntensity = 0, opacity = 1 } = {}) {
  const key = `s:${color}:${roughness}:${metalness}:${emissive}:${emissiveIntensity}:${opacity}`;
  let material = cache.get(key);
  if (!material) {
    material = new THREE.MeshStandardMaterial({
      color,
      roughness,
      metalness,
      emissive,
      emissiveIntensity,
      flatShading: true,
      transparent: opacity < 1,
      opacity,
    });
    cache.set(key, material);
  }
  return material;
}

/** Self-lit material for lights, eyes, traces and anything that should glow. */
export function glow(color, intensity = 1.4, opacity = 1) {
  return solid(color, { roughness: 0.4, metalness: 0, emissive: color, emissiveIntensity: intensity, opacity });
}

/** Translucent material used for ghosts, blueprints and beams. */
export function ghost(color, opacity = 0.35) {
  const key = `g:${color}:${opacity}`;
  let material = cache.get(key);
  if (!material) {
    material = new THREE.MeshBasicMaterial({ color, transparent: true, opacity, depthWrite: false });
    cache.set(key, material);
  }
  return material;
}

export function wire(color, opacity = 0.9) {
  const key = `w:${color}:${opacity}`;
  let material = cache.get(key);
  if (!material) {
    material = new THREE.MeshBasicMaterial({ color, wireframe: true, transparent: opacity < 1, opacity });
    cache.set(key, material);
  }
  return material;
}

export function disposeMaterialCache() {
  for (const material of cache.values()) material.dispose();
  cache.clear();
}
