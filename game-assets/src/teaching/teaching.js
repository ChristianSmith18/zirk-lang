import * as THREE from 'three';
import { PALETTE } from '../palette.js';
import { solid, glow, ghost, wire } from '../materials.js';
import { Asset, box, cylinder, octa, torus, roundBox, plane, sphere, mesh } from '../shapes.js';

/**
 * Props for the object-oriented lessons. They are the visual vocabulary:
 * a class is a blueprint, an object is a bot built from it, a clone is a
 * ghost copy, an interface is a contract scroll and an object's fields are
 * the bars of an object panel.
 */

/** A blueprint table: the "class". A translucent plan floats above it. */
export function createBlueprint({ color = PALETTE.cyan } = {}) {
  const bp = new Asset('blueprint');
  bp.add(mesh(roundBox(0.9, 0.08, 0.9, 0.03), solid(PALETTE.stoneDark), [0, 0.45, 0]));
  for (const x of [-0.38, 0.38]) for (const z of [-0.38, 0.38]) bp.add(mesh(box(0.07, 0.45, 0.07), solid(PALETTE.raised, { metalness: 0.4 }), [x, 0.225, z]));
  bp.add(mesh(box(0.8, 0.012, 0.8), solid(color, { emissive: color, emissiveIntensity: 0.4 }), [0, 0.5, 0], { shadow: false }));
  const plan = new THREE.Group();
  plan.position.y = 0.95;
  plan.add(mesh(box(0.42, 0.42, 0.42), wire(color, 0.9), [0, 0, 0], { shadow: false }));
  plan.add(mesh(box(0.3, 0.3, 0.3), ghost(color, 0.18), [0, 0, 0], { shadow: false }));
  plan.add(mesh(octa(0.06), glow(PALETTE.text, 1.4), [0, 0, 0], { shadow: false }));
  bp.add(plan);
  bp.animate = (dt, t) => {
    plan.rotation.y = t * 0.8;
    plan.position.y = 0.95 + Math.sin(t * 2) * 0.04;
  };
  return bp;
}

/**
 * The clone machine: two pods and a beam between them. `progress` 0..1 moves
 * the scanning beam; use it while a `clone()` is being copied.
 */
export function createCloner({ color = PALETTE.violet } = {}) {
  const cloner = new Asset('cloner');
  for (const x of [-0.9, 0.9]) {
    cloner.add(mesh(cylinder(0.42, 0.5, 0.1, 10), solid(PALETTE.stoneDark), [x, 0.05, 0]));
    cloner.add(mesh(cylinder(0.34, 0.34, 0.03, 10), glow(color, 0.9), [x, 0.115, 0], { shadow: false }));
    cloner.add(mesh(cylinder(0.4, 0.4, 0.9, 12), ghost(color, 0.12), [x, 0.55, 0], { shadow: false }));
    cloner.add(mesh(torus(0.4, 0.02, 24), glow(color, 1.4), [x, 1.0, 0], { rotation: [Math.PI / 2, 0, 0], shadow: false }));
  }
  cloner.add(mesh(roundBox(1.0, 0.14, 0.26, 0.04), solid(PALETTE.raised, { metalness: 0.5 }), [0, 1.08, 0]));
  const scan = mesh(box(1.1, 0.02, 0.02), glow(PALETTE.cyan, 2.2), [0, 0.5, 0], { shadow: false });
  cloner.add(scan);
  cloner.progress = 0;
  cloner.setProgress = (p) => {
    cloner.progress = THREE.MathUtils.clamp(p, 0, 1);
    scan.position.y = 0.15 + cloner.progress * 0.75;
    scan.visible = cloner.progress > 0 && cloner.progress < 1;
  };
  cloner.setProgress(0);
  return cloner;
}

/** A contract scroll: the "interface". `unroll(p)` reveals the clauses. */
export function createContractScroll({ color = PALETTE.pink } = {}) {
  const scroll = new Asset('contract-scroll');
  scroll.add(mesh(cylinder(0.07, 0.07, 0.5, 10), solid(PALETTE.wood), [0, 0.5, 0], { rotation: [0, 0, Math.PI / 2] }));
  const paper = new THREE.Group();
  const sheet = mesh(plane(0.44, 0.5), solid(PALETTE.text, { roughness: 0.95 }), [0, 0, 0], { shadow: false });
  paper.add(sheet);
  paper.position.set(0, 0.25, 0.04);
  const clauses = [];
  [0.16, 0.04, -0.08, -0.2].forEach((y, i) => {
    const w = 0.3 - (i % 2) * 0.08;
    const clause = mesh(box(w, 0.03, 0.004), glow(color, 0.7), [-0.19 + w / 2 + 0.03 - 0.0, y, 0.004], { shadow: false });
    clauses.push(clause);
    paper.add(clause);
  });
  scroll.add(paper);
  scroll.add(mesh(cylinder(0.07, 0.07, 0.5, 10), solid(PALETTE.wood), [0, 0.0, 0.04], { rotation: [0, 0, Math.PI / 2] }));
  scroll.add(mesh(octa(0.05), glow(color, 1.6), [0.28, 0.5, 0], { shadow: false }));
  scroll.unroll = (progress = 1) => {
    const p = THREE.MathUtils.clamp(progress, 0.02, 1);
    paper.scale.y = p;
    paper.position.y = 0.5 - 0.25 * p;
    clauses.forEach((clause, i) => {
      clause.visible = p > (i + 1) / (clauses.length + 1);
    });
  };
  scroll.unroll(1);
  return scroll;
}

/**
 * A floating panel for one object's state: one bar per field. Use
 * `setField(index, ratio, color?)` while tracing `fieldWrite` events.
 */
export function createObjectPanel({ fields = 3, color = PALETTE.violet } = {}) {
  const panel = new Asset('object-panel');
  const height = 0.2 + fields * 0.17;
  panel.add(mesh(roundBox(0.82, height, 0.06, 0.03), solid(PALETTE.raised, { metalness: 0.3 }), [0, height / 2 + 0.12, 0]));
  panel.add(mesh(box(0.82, 0.05, 0.065), glow(color, 1.4), [0, height + 0.12, 0], { shadow: false }));
  panel.add(mesh(cylinder(0.025, 0.025, 0.12, 6), solid(PALETTE.stone), [0, 0.06, 0]));
  panel.add(mesh(cylinder(0.14, 0.17, 0.04, 10), solid(PALETTE.stoneDark), [0, 0.02, 0]));
  const bars = [];
  for (let i = 0; i < fields; i++) {
    const y = height + 0.12 - 0.17 * (i + 1) + 0.04;
    panel.add(mesh(box(0.62, 0.06, 0.02), solid(PALETTE.surface), [0, y, 0.035], { shadow: false }));
    // The holder sits at the bar's left edge, so scaling it in x grows the
    // fill to the right; the unit-wide fill is stretched to the bar width.
    const holder = new THREE.Group();
    holder.position.set(-0.31, y, 0.05);
    const fill = mesh(box(1, 0.06, 0.02), glow(color, 1.2), [0.31, 0, 0], { shadow: false });
    fill.scale.x = 0.62;
    holder.add(fill);
    holder.scale.x = 0.001;
    panel.add(holder);
    bars.push(holder);
  }
  panel.fieldCount = fields;
  panel.setField = (index, ratio) => {
    const holder = bars[index];
    if (!holder) throw new RangeError(`object panel has ${fields} fields, got index ${index}`);
    holder.scale.x = Math.max(0.001, THREE.MathUtils.clamp(ratio, 0, 1));
  };
  panel.position.y = 0;
  return panel;
}

/** A sphere that follows another object's identity: `is` between two refs. */
export function createReferenceLink({ color = PALETTE.aqua } = {}) {
  const link = new Asset('reference-link');
  link.add(mesh(sphere(0.05, 8), glow(color, 1.6), [0, 0.5, 0], { shadow: false }));
  link.add(mesh(box(0.02, 0.5, 0.02), ghost(color, 0.6), [0, 0.25, 0], { shadow: false }));
  link.animate = (dt, t) => {
    link.children[0].position.y = 0.5 + Math.sin(t * 3) * 0.04;
  };
  return link;
}
