import test from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from 'three';
import {
  CATALOG,
  CATEGORIES,
  createAsset,
  createBot,
  createBotGhost,
  createTile,
  createCoin,
  createDoor,
  createChest,
  createBattery,
  createPlate,
  createCheckpoint,
  createCloner,
  createObjectPanel,
  createContractScroll,
  createSparkles,
  BOT_KINDS,
  TILE_TYPES,
} from '../src/index.js';

const bounds = (object) => new THREE.Box3().setFromObject(object);
const finite = (box) => [box.min, box.max].every((v) => Number.isFinite(v.x) && Number.isFinite(v.y) && Number.isFinite(v.z));

test('the catalog has unique ids, valid categories and a create function', () => {
  const ids = new Set();
  for (const asset of CATALOG) {
    assert.ok(!ids.has(asset.id), `duplicate id ${asset.id}`);
    ids.add(asset.id);
    assert.ok(CATEGORIES.includes(asset.category), `${asset.id}: bad category ${asset.category}`);
    assert.equal(typeof asset.create, 'function');
    assert.ok(Array.isArray(asset.footprint) && asset.footprint.length === 2);
  }
  for (const category of CATEGORIES) assert.ok(CATALOG.some((a) => a.category === category), `empty category ${category}`);
});

test('every asset builds a non-empty, finite, game-scale model', () => {
  for (const asset of CATALOG) {
    const object = asset.create();
    // Effects such as sparkles start as a point; measure them mid-animation.
    if (asset.category === 'effects') object.update?.(0.3);
    assert.ok(object instanceof THREE.Object3D, asset.id);
    let meshes = 0;
    object.traverse((o) => (o.isMesh || o.isPoints) && meshes++);
    assert.ok(meshes > 0, `${asset.id} has no geometry`);
    const box = bounds(object);
    assert.ok(finite(box), `${asset.id} bounding box is not finite`);
    const size = box.getSize(new THREE.Vector3());
    // One tile is 1 unit; nothing should be tiny or absurdly large.
    // Effects are floor decals or particles and may be nearly flat.
    const flat = asset.category === 'effects';
    assert.ok((flat || size.y > 0.02) && size.y < 3.2, `${asset.id} height ${size.y.toFixed(2)}`);
    assert.ok(Math.max(size.x, size.z) < 3.4, `${asset.id} footprint ${size.x.toFixed(2)}x${size.z.toFixed(2)}`);
  }
});

test('animated assets stay finite over a long run', () => {
  for (const asset of CATALOG) {
    const object = asset.create();
    if (!object.update) continue;
    for (let i = 0; i < 600; i++) object.update(1 / 60);
    assert.ok(finite(bounds(object)), `${asset.id} went non-finite while animating`);
  }
});

test('assets with the same options are deterministic', () => {
  for (const type of TILE_TYPES) {
    const a = createTile(type, { variant: 3 });
    const b = createTile(type, { variant: 3 });
    const positions = (o) => {
      const out = [];
      o.traverse((c) => out.push([c.position.x, c.position.y, c.position.z].map((n) => n.toFixed(5)).join(',')));
      return out;
    };
    assert.deepEqual(positions(a), positions(b), `tile ${type} varies between builds`);
  }
  // Different variants differ for the procedural tiles.
  const x = createTile('circuit', { variant: 1 });
  const y = createTile('circuit', { variant: 2 });
  assert.notEqual(x.children.length === y.children.length && JSON.stringify(x.children.map((c) => c.position)), JSON.stringify(y.children.map((c) => c.position)));
});

test('tiles keep the walkable surface at y = 0', () => {
  for (const type of TILE_TYPES) {
    if (['water', 'lava', 'hole', 'bridge'].includes(type)) continue;
    const box = bounds(createTile(type));
    assert.ok(box.max.y >= -0.001 && box.max.y < 0.55, `${type} surface at ${box.max.y.toFixed(3)}`);
    assert.ok(Math.abs(box.min.x + 0.5) < 0.02 || Math.abs(box.max.x - 0.5) < 0.02, `${type} is not one tile wide`);
  }
});

test('bots: every kind builds, every animation plays, one-shots return to idle', () => {
  for (const kind of Object.keys(BOT_KINDS)) {
    const bot = createBot(kind);
    for (const name of ['idle', 'walk', 'jump', 'celebrate', 'sad', 'think']) {
      bot.play(name);
      for (let i = 0; i < 120; i++) bot.update(1 / 60);
      assert.ok(finite(bounds(bot)), `${kind}/${name} went non-finite`);
    }
    bot.play('jump');
    for (let i = 0; i < 120; i++) bot.update(1 / 60);
    assert.equal(bot.animation, 'idle', `${kind} jump should end in idle`);
    bot.play('celebrate');
    for (let i = 0; i < 200; i++) bot.update(1 / 60);
    assert.equal(bot.animation, 'idle', `${kind} celebrate should end in idle`);
  }
  assert.throws(() => createBot('nope'), /unknown bot kind/);
  assert.throws(() => createBot('zirk').play('fly'), /unknown animation/);
});

test('bots jump up and come back down', () => {
  const bot = createBot('zirk');
  bot.play('jump');
  let peak = 0;
  for (let i = 0; i < 60; i++) {
    bot.update(1 / 60);
    peak = Math.max(peak, bot.rig.position.y);
  }
  assert.ok(peak > 0.3, `jump peak ${peak}`);
  for (let i = 0; i < 90; i++) bot.update(1 / 60);
  assert.ok(Math.abs(bot.rig.position.y) < 0.05, 'bot should land');
});

test('the clone ghost is translucent and shares the skeleton', () => {
  const ghost = createBotGhost('zirk');
  let translucent = 0;
  ghost.traverse((o) => {
    if (o.isMesh && o.material.transparent) translucent++;
  });
  assert.ok(translucent > 5, 'ghost should use translucent materials');
  assert.equal(typeof ghost.play, 'function');
});

test('state-driven assets clamp their inputs', () => {
  const door = createDoor();
  door.open(5);
  const openY = door.children.find((c) => c.isGroup && c.children.length > 5).position.y;
  door.open(-3);
  const closedY = door.children.find((c) => c.isGroup && c.children.length > 5).position.y;
  assert.ok(openY < closedY && Number.isFinite(openY));
  const battery = createBattery({ level: 0.5 });
  battery.setLevel(9);
  battery.setLevel(-1);
  assert.ok(finite(bounds(battery)));
  const chest = createChest();
  chest.open(2);
  chest.open(-2);
  createPlate().press(true);
  createCheckpoint().activate(true);
  createCloner().setProgress(3);
  createContractScroll().unroll(-1);
});

test('object panel validates field indexes and keeps bars inside the panel', () => {
  const panel = createObjectPanel({ fields: 3 });
  assert.equal(panel.fieldCount, 3);
  panel.setField(0, 2);
  panel.setField(2, -1);
  assert.throws(() => panel.setField(3, 0.5), RangeError);
  assert.ok(bounds(panel).getSize(new THREE.Vector3()).x < 1);
});

test('shared materials are reused between instances', () => {
  const a = createCoin();
  const b = createCoin();
  const materials = (o) => {
    const set = new Set();
    o.traverse((c) => c.isMesh && set.add(c.material));
    return set;
  };
  const shared = [...materials(a)].filter((m) => materials(b).has(m));
  assert.ok(shared.length >= 2, 'coins should share cached materials');
});

test('sparkles finish and stop drawing', () => {
  const fx = createSparkles({ lifetime: 0.5 });
  assert.equal(fx.children[0].frustumCulled, false, 'moving particles must not be frustum-culled');
  assert.equal(fx.finished, false);
  for (let i = 0; i < 60; i++) fx.update(1 / 60);
  assert.equal(fx.finished, true);
  fx.dispose();
});

test('createAsset resolves ids and rejects unknown ones', () => {
  assert.ok(createAsset('goal-portal') instanceof THREE.Object3D);
  assert.throws(() => createAsset('nope'), /unknown asset/);
});

test('assets can be exported with GLTFExporter-compatible scene graphs', () => {
  // The exporter walks standard Object3D/Mesh data; make sure nothing relies on
  // non-serializable constructs such as custom Mesh subclasses.
  for (const asset of CATALOG) {
    createAsset(asset.id).traverse((o) => {
      assert.ok(!o.isSkinnedMesh && !o.isInstancedMesh, `${asset.id} uses an unsupported mesh type`);
    });
  }
});

test('the coin carries the real Zirk mark on both faces, with the logo proportions', async () => {
  const { createZirkMarkGeometries, ZIRK_MARK } = await import('../src/logo.js');
  const { z, slash } = createZirkMarkGeometries({ size: 0.33, depth: 0.012 });
  for (const geometry of [z, slash]) {
    assert.ok(geometry.attributes.color, 'the mark is coloured per vertex (brand gradients)');
    geometry.computeBoundingBox();
  }
  // Compare with the SVG: strokes extended by half the stroke width at both ends.
  const half = ZIRK_MARK.stroke / 2;
  const xs = [...ZIRK_MARK.z, ...ZIRK_MARK.slash].map((p) => p[0]);
  const ys = [...ZIRK_MARK.z, ...ZIRK_MARK.slash].map((p) => p[1]);
  const expectedAspect = (Math.max(...xs) - Math.min(...xs) + 2 * half) / (Math.max(...ys) - Math.min(...ys) + 2 * half);
  const union = z.boundingBox.clone().union(slash.boundingBox);
  const size = union.getSize(new THREE.Vector3());
  assert.ok(Math.abs(size.x / size.y - expectedAspect) < 0.03, `aspect ${(size.x / size.y).toFixed(3)} vs ${expectedAspect.toFixed(3)}`);
  // The slash is drawn over the Z, and the Z's top-left stop is violet while its end is blue.
  const first = new THREE.Color().fromBufferAttribute(z.attributes.color, 0);
  assert.ok(first.b > first.g, 'the Z starts in the violet/blue range');

  const coin = createCoin();
  let marks = 0;
  coin.traverse((o) => o.isMesh && o.material.vertexColors && marks++);
  assert.equal(marks, 4, 'two strokes on each of the two faces');
  const box = bounds(coin);
  assert.ok(box.max.y - box.min.y < 0.6, 'the mark must fit inside the coin');
});
